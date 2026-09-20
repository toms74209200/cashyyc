import json
import os
import random
import string
import subprocess
import time
from pathlib import Path

import pytest
from pytest_bdd import given, parsers, then, when

from lib.tracing import StepTimer, TestTiming, get_collector, render_table

REPO_ROOT = Path(__file__).resolve().parents[2]

_SPAN_REPORT_OPTION = "--span-report"
_CYYC_BIN_OPTION = "--cyyc-bin"
_REPORT_FILE = Path(__file__).parent / "reports" / "spanTiming" / "spanTiming.txt"
_STOP_SETTLE_TIMEOUT = 30.0
_STOP_SETTLE_INTERVAL = 0.1
_step_timer = StepTimer()
_span_report_enabled = False


def pytest_addoption(parser: pytest.Parser) -> None:
    parser.addoption(_SPAN_REPORT_OPTION, action="store_true", default=False)
    parser.addoption(_CYYC_BIN_OPTION, action="store", default=None)


def pytest_configure(config: pytest.Config) -> None:
    global _span_report_enabled
    _span_report_enabled = config.getoption(_SPAN_REPORT_OPTION)


def pytest_bdd_before_step_call(
    request, feature, scenario, step, step_func, step_func_args
):
    if _span_report_enabled:
        _step_timer.start_step(step.type)


def pytest_bdd_after_step(request, feature, scenario, step, step_func, step_func_args):
    if _span_report_enabled:
        _step_timer.end_step()


def pytest_bdd_step_error(
    request, feature, scenario, step, step_func, step_func_args, exception
):
    if _span_report_enabled:
        _step_timer.end_step()


def pytest_bdd_after_scenario(request, feature, scenario):
    if not _span_report_enabled:
        return
    nanos = _step_timer.collect_phase_nanos()
    get_collector().record(
        TestTiming(
            test_module=request.node.module.__name__,
            test_name=request.node.name,
            given_nanos=nanos["given"],
            when_nanos=nanos["when"],
            then_nanos=nanos["then"],
        )
    )


def pytest_sessionfinish(session: pytest.Session, exitstatus: int) -> None:
    if not _span_report_enabled:
        return
    timings = get_collector().all()
    if not timings:
        return
    table = render_table(timings)
    print(f"\n{table}")
    _REPORT_FILE.parent.mkdir(parents=True, exist_ok=True)
    _REPORT_FILE.write_text(table)


def _random_name(length: int = 8) -> str:
    return "".join(random.choices(string.ascii_lowercase + string.digits, k=length))


@pytest.fixture(scope="session")
def cyyc_binary(request):
    bin_option = request.config.getoption(_CYYC_BIN_OPTION)
    if bin_option:
        bin_path = Path(bin_option).resolve()
        assert bin_path.is_file(), f"binary not found: {bin_path}"
        return bin_path
    subprocess.run(
        ["cargo", "build", "--bin", "cyyc"],
        cwd=REPO_ROOT,
        check=True,
    )
    return REPO_ROOT / "target" / "debug" / "cyyc"


@pytest.fixture
def workspace(tmp_path):
    workspace_dir = tmp_path / _random_name()
    devcontainer_dir = workspace_dir / ".devcontainer"
    devcontainer_dir.mkdir(parents=True)
    yield workspace_dir
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
            f"{workspace_dir.name}_devcontainer",
            "down",
            "-v",
            "--remove-orphans",
        ],
        capture_output=True,
    )
    name = workspace_dir.name.lower()
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


def container_ids_by_devcontainer(
    workspace: Path, *, all_states: bool = False
) -> list[str]:
    flag = "-aq" if all_states else "-q"
    ids: list[str] = []
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
        ids.extend(r.stdout.split())
    return ids


def container_id_by_devcontainer(
    workspace: Path, *, all_states: bool = False
) -> str | None:
    ids = container_ids_by_devcontainer(workspace, all_states=all_states)
    return ids[0] if ids else None


def container_ids_by_compose(workspace: Path, *, all_states: bool = False) -> list[str]:
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
    return out.split("\n") if out else []


def _container_ids(
    workspace: Path, config: dict, *, all_states: bool = False
) -> list[str]:
    if "dockerComposeFile" in config:
        return container_ids_by_compose(workspace, all_states=all_states)
    return container_ids_by_devcontainer(workspace, all_states=all_states)


def _container_id(
    workspace: Path, config: dict, *, all_states: bool = False
) -> str | None:
    ids = _container_ids(workspace, config, all_states=all_states)
    return ids[0] if ids else None


def _wait_until_no_container_runs(workspace: Path, config: dict) -> None:
    deadline = time.monotonic() + _STOP_SETTLE_TIMEOUT
    while True:
        running = _container_ids(workspace, config)
        if not running:
            return
        if time.monotonic() >= deadline:
            states = subprocess.run(
                ["docker", "inspect", "-f", "{{.Id}} {{.State.Status}}", *running],
                capture_output=True,
                text=True,
            ).stdout.strip()
            raise AssertionError(
                "precondition failed: container still runs "
                f"{_STOP_SETTLE_TIMEOUT}s after `docker stop`:\n{states}"
            )
        time.sleep(_STOP_SETTLE_INTERVAL)


@given(
    parsers.parse('a devcontainer config with image "{image}"'), target_fixture="config"
)
def given_image_config(workspace, image):
    config = {"image": image}
    (workspace / ".devcontainer" / "devcontainer.json").write_text(json.dumps(config))
    return config


@given("an image built from:", target_fixture="base_image")
def given_image_built_from(workspace, docstring):
    build_dir = workspace / "base-image"
    build_dir.mkdir(parents=True, exist_ok=True)
    (build_dir / "Dockerfile").write_text(docstring)
    tag = f"vsc-{workspace.name.lower()}-base"
    result = subprocess.run(
        ["docker", "build", "-t", tag, str(build_dir)],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, f"docker build failed: {result.stderr}"
    return tag


@given("a devcontainer config based on that image", target_fixture="config")
def given_config_based_on_base_image(workspace, base_image):
    (workspace / ".devcontainer" / "Dockerfile").write_text(f"FROM {base_image}\n")
    config = {"dockerFile": "Dockerfile"}
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


@given(
    parsers.parse(
        'a devcontainer config using docker-compose service "{service}" with runService "{run_service}" and image "{image}"'
    ),
    target_fixture="config",
)
def given_compose_config_with_run_service(workspace, service, run_service, image):
    compose_yaml = (
        f"services:\n"
        f"  {service}:\n"
        f"    image: {image}\n"
        f"    command: sleep infinity\n"
        f"    volumes:\n"
        f"      - ..:/workspaces:cached\n"
        f"    working_dir: /workspaces\n"
        f"  {run_service}:\n"
        f"    image: {image}\n"
        f"    command: sleep infinity\n"
    )
    (workspace / ".devcontainer" / "docker-compose.yml").write_text(compose_yaml)
    config = {
        "dockerComposeFile": "docker-compose.yml",
        "service": service,
        "runServices": [run_service],
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
    cids = _container_ids(workspace, config, all_states=True)
    assert cids, "precondition failed: container was not created"
    container_id_before[0] = cids[0]
    subprocess.run(["docker", "stop", *cids], capture_output=True, check=True)
    _wait_until_no_container_runs(workspace, config)


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


@given(
    parsers.parse('the config has remoteUser "{value}"'),
    target_fixture="config",
)
def given_remote_user(workspace, config, value):
    new_config = {**config, "remoteUser": value}
    (workspace / ".devcontainer" / "devcontainer.json").write_text(
        json.dumps(new_config)
    )
    return new_config


@given(
    parsers.parse("the config has updateRemoteUserUID false"),
    target_fixture="config",
)
def given_update_remote_user_uid_false(workspace, config):
    new_config = {**config, "updateRemoteUserUID": False}
    (workspace / ".devcontainer" / "devcontainer.json").write_text(
        json.dumps(new_config)
    )
    return new_config


@given(
    parsers.parse("the config has init true"),
    target_fixture="config",
)
def given_init_true(workspace, config):
    new_config = {**config, "init": True}
    (workspace / ".devcontainer" / "devcontainer.json").write_text(
        json.dumps(new_config)
    )
    return new_config


@given(
    parsers.parse("the config has privileged true"),
    target_fixture="config",
)
def given_privileged_true(workspace, config):
    new_config = {**config, "privileged": True}
    (workspace / ".devcontainer" / "devcontainer.json").write_text(
        json.dumps(new_config)
    )
    return new_config


@given(
    parsers.parse('the config has containerEnv "{key}" set to "{value}"'),
    target_fixture="config",
)
def given_container_env_key_value(workspace, config, key, value):
    container_env = {**config.get("containerEnv", {}), key: value}
    new_config = {**config, "containerEnv": container_env}
    (workspace / ".devcontainer" / "devcontainer.json").write_text(
        json.dumps(new_config)
    )
    return new_config


@given(
    parsers.parse('the config has remoteEnv "{key}" set to "{value}"'),
    target_fixture="config",
)
def given_remote_env_key_value(workspace, config, key, value):
    remote_env = {**config.get("remoteEnv", {}), key: value}
    new_config = {**config, "remoteEnv": remote_env}
    (workspace / ".devcontainer" / "devcontainer.json").write_text(
        json.dumps(new_config)
    )
    return new_config


@given(
    parsers.parse('the config has workspaceFolder "{value}"'),
    target_fixture="config",
)
def given_workspace_folder_config(workspace, config, value):
    new_config = {**config, "workspaceFolder": value}
    (workspace / ".devcontainer" / "devcontainer.json").write_text(
        json.dumps(new_config)
    )
    return new_config


@given(
    parsers.parse('the config has workspaceMount "{value}"'),
    target_fixture="config",
)
def given_workspace_mount_config(workspace, config, value):
    new_config = {**config, "workspaceMount": value}
    (workspace / ".devcontainer" / "devcontainer.json").write_text(
        json.dumps(new_config)
    )
    return new_config


@given(
    parsers.parse('the config has mounts with "{value}"'),
    target_fixture="config",
)
def given_config_mounts(workspace, config, value):
    mounts = [*config.get("mounts", []), value]
    new_config = {**config, "mounts": mounts}
    (workspace / ".devcontainer" / "devcontainer.json").write_text(
        json.dumps(new_config)
    )
    return new_config


@given(
    parsers.parse(
        'the config has an object-form mount with type "{mount_type}", source "{source}" and target "{target}"'
    ),
    target_fixture="config",
)
def given_config_object_form_mount(workspace, config, mount_type, source, target):
    mount = {"type": mount_type, "source": source, "target": target}
    mounts = [*config.get("mounts", []), mount]
    new_config = {**config, "mounts": mounts}
    (workspace / ".devcontainer" / "devcontainer.json").write_text(
        json.dumps(new_config)
    )
    return new_config


@given(
    parsers.parse('the config has runArgs with env "{key}" set to "{value}"'),
    target_fixture="config",
)
def given_run_args_env(workspace, config, key, value):
    run_args = [*config.get("runArgs", []), "--env", f"{key}={value}"]
    new_config = {**config, "runArgs": run_args}
    (workspace / ".devcontainer" / "devcontainer.json").write_text(
        json.dumps(new_config)
    )
    return new_config


@given(
    parsers.parse('the config has runArgs with user "{value}"'),
    target_fixture="config",
)
def given_run_args_user(workspace, config, value):
    run_args = [*config.get("runArgs", []), "-u", value]
    new_config = {**config, "runArgs": run_args}
    (workspace / ".devcontainer" / "devcontainer.json").write_text(
        json.dumps(new_config)
    )
    return new_config


@given(
    parsers.parse('the config has containerUser "{value}"'),
    target_fixture="config",
)
def given_container_user_config(workspace, config, value):
    new_config = {**config, "containerUser": value}
    (workspace / ".devcontainer" / "devcontainer.json").write_text(
        json.dumps(new_config)
    )
    return new_config


@given("the config has a Dockerfile that adds the host user", target_fixture="config")
def given_dockerfile_adds_host_user(workspace, config):
    image = config.get("image", "mcr.microsoft.com/devcontainers/base:debian")
    current_user = os.environ.get("USER", "vscode")
    dockerfile = (
        f"FROM {image}\n"
        f"RUN id {current_user} 2>/dev/null || useradd -ms /bin/bash {current_user}\n"
    )
    (workspace / ".devcontainer" / "Dockerfile").write_text(dockerfile)
    new_config = {k: v for k, v in config.items() if k != "image"}
    new_config["dockerFile"] = "Dockerfile"
    (workspace / ".devcontainer" / "devcontainer.json").write_text(
        json.dumps(new_config)
    )
    return new_config
