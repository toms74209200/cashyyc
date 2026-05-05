import json
import subprocess

from pytest_bdd import given, parsers, scenarios, then, when

from conftest import (
    container_id_by_compose,
    container_id_by_devcontainer,
)

scenarios("../../features/shell.feature")


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


@given(parsers.parse('the config has feature "{feature_id}"'), target_fixture="config")
def given_has_feature(workspace, config, feature_id):
    features = {**config.get("features", {}), feature_id: {}}
    new_config = {**config, "features": features}
    (workspace / ".devcontainer" / "devcontainer.json").write_text(
        json.dumps(new_config)
    )
    return new_config


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
    if "dockerComposeFile" in config:
        cid = container_id_by_compose(workspace, all_states=True)
    else:
        cid = container_id_by_devcontainer(workspace, all_states=True)
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
    if "dockerComposeFile" in config:
        cid = container_id_by_compose(workspace)
    else:
        cid = container_id_by_devcontainer(workspace)
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


@then("the container is running")
def then_container_running(workspace, config):
    if "dockerComposeFile" in config:
        cid = container_id_by_compose(workspace)
    else:
        cid = container_id_by_devcontainer(workspace)
    assert cid, "no running container found"


@then("the existing container is reused")
def then_existing_reused(workspace, config, container_id_before):
    if "dockerComposeFile" in config:
        cid_now = container_id_by_compose(workspace)
    else:
        cid_now = container_id_by_devcontainer(workspace)
    assert cid_now == container_id_before[0], (
        f"expected reused {container_id_before[0]}, got {cid_now}"
    )


@then("a new shell session is opened in the existing container")
def then_new_session_in_existing(workspace, config, container_id_before):
    if "dockerComposeFile" in config:
        cid_now = container_id_by_compose(workspace)
    else:
        cid_now = container_id_by_devcontainer(workspace)
    assert cid_now == container_id_before[0], (
        f"expected reused {container_id_before[0]}, got {cid_now}"
    )


@then(parsers.parse('the command "{cmd}" succeeds in the resulting shell'))
def then_command_succeeds(workspace, config, cmd):
    if "dockerComposeFile" in config:
        container_id = container_id_by_compose(workspace)
    else:
        container_id = container_id_by_devcontainer(workspace)
    assert container_id, "no running container found"
    result = subprocess.run(
        ["docker", "exec", container_id, "sh", "-c", cmd],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, (
        f"command failed (stdout={result.stdout!r}, stderr={result.stderr!r})"
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
