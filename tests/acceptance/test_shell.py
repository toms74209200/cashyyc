import json
import subprocess

from pytest_bdd import given, parsers, scenarios, then

from conftest import (
    container_id_by_compose,
    container_id_by_devcontainer,
)

scenarios("../../features/shell.feature")


@given(parsers.parse('the config has feature "{feature_id}"'), target_fixture="config")
def given_has_feature(workspace, config, feature_id):
    features = {**config.get("features", {}), feature_id: {}}
    new_config = {**config, "features": features}
    (workspace / ".devcontainer" / "devcontainer.json").write_text(
        json.dumps(new_config)
    )
    return new_config


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
