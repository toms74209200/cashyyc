import json
import subprocess
import time

from pytest_bdd import given, parsers, scenarios, then

from conftest import (
    _container_id,
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


@given(
    parsers.re(r"the config has (?P<field>\w+Command) (?P<cmd_json>.+)"),
    target_fixture="config",
)
def given_lifecycle_command(workspace, config, field, cmd_json):
    cmd = json.loads(cmd_json)
    new_config = {**config, field: cmd}
    (workspace / ".devcontainer" / "devcontainer.json").write_text(
        json.dumps(new_config)
    )
    return new_config


@then(parsers.parse('the file "{path}" exists in the workspace'))
def then_file_exists_in_workspace(workspace, path):
    assert (workspace / path).exists(), f"file {path!r} not found in workspace"


@then(parsers.parse('the file "{path}" does not exist in the workspace'))
def then_file_not_exists_in_workspace(workspace, path):
    assert not (workspace / path).exists(), (
        f"file {path!r} unexpectedly found in workspace"
    )


@then(parsers.parse('the file "{path}" exists in the container'))
def then_file_exists_in_container(workspace, config, path):
    container_id = _container_id(workspace, config)
    assert container_id, "no running container found"
    result = subprocess.run(
        ["docker", "exec", container_id, "test", "-f", path],
        capture_output=True,
    )
    assert result.returncode == 0, f"file {path!r} not found in container"


@given(
    parsers.re(r'the config has waitFor "(?P<value>\w+)"'),
    target_fixture="config",
)
def given_wait_for(workspace, config, value):
    new_config = {**config, "waitFor": value}
    (workspace / ".devcontainer" / "devcontainer.json").write_text(
        json.dumps(new_config)
    )
    return new_config


@then(parsers.parse('the file "{path}" eventually exists in the container'))
def then_file_eventually_exists_in_container(workspace, config, path):
    container_id = _container_id(workspace, config)
    assert container_id, "no running container found"
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        result = subprocess.run(
            ["docker", "exec", container_id, "test", "-f", path],
            capture_output=True,
        )
        if result.returncode == 0:
            return
        time.sleep(0.5)
    raise AssertionError(f"file {path!r} not found in container after 10s")


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


@then(parsers.parse('the container user "{user}" UID matches the host UID'))
def then_container_user_uid_matches_host(workspace, config, user):
    if "dockerComposeFile" in config:
        container_id = container_id_by_compose(workspace)
    else:
        container_id = container_id_by_devcontainer(workspace)
    assert container_id, "no running container found"
    host_uid = subprocess.run(
        ["id", "-u"], capture_output=True, text=True, check=True
    ).stdout.strip()
    container_uid = subprocess.run(
        ["docker", "exec", container_id, "id", "-u", user],
        capture_output=True,
        text=True,
    ).stdout.strip()
    assert container_uid == host_uid, (
        f"expected container user {user!r} UID {container_uid!r} to match host UID {host_uid!r}"
    )


@then(parsers.parse('the container user "{user}" UID is "{expected_uid}"'))
def then_container_user_uid_is(workspace, config, user, expected_uid):
    if "dockerComposeFile" in config:
        container_id = container_id_by_compose(workspace)
    else:
        container_id = container_id_by_devcontainer(workspace)
    assert container_id, "no running container found"
    container_uid = subprocess.run(
        ["docker", "exec", container_id, "id", "-u", user],
        capture_output=True,
        text=True,
    ).stdout.strip()
    assert container_uid == expected_uid, (
        f"expected container user {user!r} UID {container_uid!r} to be {expected_uid!r}"
    )


@given(parsers.parse("the config has appPort {value}"), target_fixture="config")
def given_app_port(workspace, config, value):
    app_port = json.loads(value)
    new_config = {**config, "appPort": app_port}
    (workspace / ".devcontainer" / "devcontainer.json").write_text(
        json.dumps(new_config)
    )
    return new_config


@then(parsers.parse("the container has port {port:d} bound to {host_ip}"))
def then_port_bound_to(workspace, config, port, host_ip):
    cid = _container_id(workspace, config)
    assert cid, "no running container found"
    result = subprocess.run(
        ["docker", "port", cid, str(port)],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0 and host_ip in result.stdout, (
        f"port {port} not bound to {host_ip}: {result.stdout!r}"
    )


@then(parsers.parse("the container has port {port:d} bound"))
def then_port_bound(workspace, config, port):
    cid = _container_id(workspace, config)
    assert cid, "no running container found"
    result = subprocess.run(
        ["docker", "port", cid, str(port)],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0 and result.stdout.strip(), f"port {port} is not bound"
