import json
import subprocess
from pathlib import Path

import pytest
from pytest_bdd import given, parsers, then, when

REPO_ROOT = Path(__file__).resolve().parents[2]


@pytest.fixture(scope="session")
def cyyc_binary():
    subprocess.run(
        ["cargo", "build", "--bin", "cyyc"],
        cwd=REPO_ROOT,
        check=True,
    )
    return REPO_ROOT / "target" / "debug" / "cyyc"


@pytest.fixture
def workspace(tmp_path):
    devcontainer_dir = tmp_path / ".devcontainer"
    devcontainer_dir.mkdir()
    yield tmp_path
    for cfg_path in devcontainer_dir.rglob("devcontainer.json"):
        r = subprocess.run(
            [
                "docker",
                "ps",
                "-aq",
                "--filter",
                f"label=devcontainer.config_file={cfg_path}",
            ],
            capture_output=True,
            text=True,
        )
        ids = r.stdout.split()
        if ids:
            subprocess.run(["docker", "rm", "-f", *ids], capture_output=True)
    subprocess.run(
        [
            "docker",
            "compose",
            "-p",
            f"{tmp_path.name}_devcontainer",
            "down",
            "-v",
            "--remove-orphans",
        ],
        capture_output=True,
    )
    name = tmp_path.name.lower()
    r = subprocess.run(
        ["docker", "images", "-q", "--filter", f"reference=vsc-{name}*"],
        capture_output=True,
        text=True,
    )
    img_ids = list(set(r.stdout.split()))
    if img_ids:
        subprocess.run(["docker", "rmi", "-f", *img_ids], capture_output=True)


@pytest.fixture
def config():
    return {}


@pytest.fixture
def named_configs():
    return {}


@pytest.fixture
def run_result():
    return {}


@pytest.fixture
def container_id_before():
    return [None]


def container_id_by_devcontainer(
    workspace: Path, *, all_states: bool = False
) -> str | None:
    flag = "-aq" if all_states else "-q"
    for cfg_path in (workspace / ".devcontainer").rglob("devcontainer.json"):
        r = subprocess.run(
            [
                "docker",
                "ps",
                flag,
                "--filter",
                f"label=devcontainer.config_file={cfg_path}",
            ],
            capture_output=True,
            text=True,
        )
        out = r.stdout.strip()
        if out:
            return out.split("\n")[0]
    return None


def container_id_by_compose(workspace: Path, *, all_states: bool = False) -> str | None:
    flag = "-aq" if all_states else "-q"
    r = subprocess.run(
        [
            "docker",
            "ps",
            flag,
            "--filter",
            f"label=com.docker.compose.project={workspace.name}_devcontainer",
        ],
        capture_output=True,
        text=True,
    )
    out = r.stdout.strip()
    if out:
        return out.split("\n")[0]
    return None


def _container_id(
    workspace: Path, config: dict, *, all_states: bool = False
) -> str | None:
    if "dockerComposeFile" in config:
        return container_id_by_compose(workspace, all_states=all_states)
    return container_id_by_devcontainer(workspace, all_states=all_states)


@given(
    parsers.parse('a devcontainer config with image "{image}"'), target_fixture="config"
)
def given_image_config(workspace, image):
    config = {"image": image}
    (workspace / ".devcontainer" / "devcontainer.json").write_text(json.dumps(config))
    return config


@given("a devcontainer config with Dockerfile:", target_fixture="config")
def given_dockerfile_config(workspace, docstring):
    (workspace / ".devcontainer" / "Dockerfile").write_text(docstring)
    config = {"dockerFile": "Dockerfile"}
    (workspace / ".devcontainer" / "devcontainer.json").write_text(json.dumps(config))
    return config


@given("a devcontainer config with build using Dockerfile:", target_fixture="config")
def given_build_config(workspace, docstring):
    (workspace / ".devcontainer" / "Dockerfile").write_text(docstring)
    config = {"build": {"dockerfile": "Dockerfile"}}
    (workspace / ".devcontainer" / "devcontainer.json").write_text(json.dumps(config))
    return config


@given(
    parsers.parse(
        'a devcontainer config using docker-compose service "{service}" with image "{image}"'
    ),
    target_fixture="config",
)
def given_compose_config(workspace, service, image):
    compose_yaml = (
        f"services:\n"
        f"  {service}:\n"
        f"    image: {image}\n"
        f"    command: sleep infinity\n"
        f"    volumes:\n"
        f"      - ..:/workspaces:cached\n"
        f"    working_dir: /workspaces\n"
    )
    (workspace / ".devcontainer" / "docker-compose.yml").write_text(compose_yaml)
    config = {
        "dockerComposeFile": "docker-compose.yml",
        "service": service,
        "workspaceFolder": "/workspaces",
    }
    (workspace / ".devcontainer" / "devcontainer.json").write_text(json.dumps(config))
    return config


@given(parsers.parse('a "{name}" devcontainer config with image "{image}"'))
def given_named_image_config(workspace, named_configs, name, image):
    config = {"image": image}
    named_dir = workspace / ".devcontainer" / name
    named_dir.mkdir(parents=True, exist_ok=True)
    (named_dir / "devcontainer.json").write_text(json.dumps(config))
    named_configs[name] = config


@given("no container exists for this config")
def given_no_container():
    pass


@given("no devcontainer config exists")
def given_no_config():
    pass


@given("a stopped container exists for this config")
def given_stopped_container(workspace, config, cyyc_binary, container_id_before):
    subprocess.run(
        [str(cyyc_binary), "shell"],
        cwd=workspace,
        stdin=subprocess.DEVNULL,
        capture_output=True,
        text=True,
        timeout=600,
    )
    cid = _container_id(workspace, config, all_states=True)
    assert cid, "precondition failed: container was not created"
    container_id_before[0] = cid
    subprocess.run(["docker", "stop", cid], capture_output=True, check=True)


@given("a running container exists for this config")
def given_running_container(workspace, config, cyyc_binary, container_id_before):
    subprocess.run(
        [str(cyyc_binary), "shell"],
        cwd=workspace,
        stdin=subprocess.DEVNULL,
        capture_output=True,
        text=True,
        timeout=600,
    )
    cid = _container_id(workspace, config)
    assert cid, "precondition failed: container is not running"
    container_id_before[0] = cid


@when(parsers.parse('running "{command}"'))
def when_running(workspace, cyyc_binary, command, run_result):
    args = command.split()
    assert args[0] == "cyyc"
    result = subprocess.run(
        [str(cyyc_binary), *args[1:]],
        cwd=workspace,
        stdin=subprocess.DEVNULL,
        capture_output=True,
        text=True,
        timeout=600,
    )
    run_result.update(
        {
            "stdout": result.stdout,
            "stderr": result.stderr,
            "returncode": result.returncode,
        }
    )


@then("the available environment names are printed")
def then_names_printed(named_configs, run_result):
    output = run_result["stdout"] + run_result["stderr"]
    for name in named_configs:
        assert name in output, f"expected env name {name!r} in output, got: {output!r}"


@then("the command exits with a non-zero status")
def then_nonzero(run_result):
    assert run_result["returncode"] != 0, (
        f"expected non-zero exit, got {run_result['returncode']}"
    )


@then("the command exits successfully")
def then_exits_successfully(run_result):
    assert run_result["returncode"] == 0, (
        f"expected zero exit, got {run_result['returncode']}: "
        f"stdout={run_result['stdout']!r}, stderr={run_result['stderr']!r}"
    )
