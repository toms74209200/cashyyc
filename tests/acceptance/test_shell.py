import json
import subprocess
import time

from pytest_bdd import given, parsers, scenarios, then

from conftest import (
    _container_id,
    container_id_by_devcontainer,
    container_ids_by_compose,
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
        ids = container_ids_by_compose(workspace)
        cid = ids[0] if ids else None
    else:
        cid = container_id_by_devcontainer(workspace)
    assert cid, "no running container found"


@then("the existing container is reused")
def then_existing_reused(workspace, config, container_id_before):
    if "dockerComposeFile" in config:
        ids = container_ids_by_compose(workspace)
        cid_now = ids[0] if ids else None
    else:
        cid_now = container_id_by_devcontainer(workspace)
    assert cid_now == container_id_before[0], (
        f"expected reused {container_id_before[0]}, got {cid_now}"
    )


@then("a new shell session is opened in the existing container")
def then_new_session_in_existing(workspace, config, container_id_before):
    if "dockerComposeFile" in config:
        ids = container_ids_by_compose(workspace)
        cid_now = ids[0] if ids else None
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
        ids = container_ids_by_compose(workspace)
        container_id = ids[0] if ids else None
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


@then(parsers.parse('the file "{path}" in the container contains "{text}"'))
def then_file_in_container_contains(workspace, config, path, text):
    container_id = _container_id(workspace, config)
    assert container_id, "no running container found"
    result = subprocess.run(
        ["docker", "exec", container_id, "cat", path],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, f"file {path!r} not found in container"
    assert text in result.stdout, (
        f"expected {text!r} in {path!r}, got {result.stdout!r}"
    )


@then(parsers.parse('the container user "{user}" UID matches the host UID'))
def then_container_user_uid_matches_host(workspace, config, user):
    if "dockerComposeFile" in config:
        ids = container_ids_by_compose(workspace)
        container_id = ids[0] if ids else None
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
        ids = container_ids_by_compose(workspace)
        container_id = ids[0] if ids else None
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


@given(
    'the config has local features "alpha" and "beta" that log their id on install',
    target_fixture="config",
)
def given_local_features_alpha_beta(workspace, config):
    features_dir = workspace / ".devcontainer" / "features"
    for name in ("alpha", "beta"):
        d = features_dir / name
        d.mkdir(parents=True, exist_ok=True)
        (d / "devcontainer-feature.json").write_text(
            json.dumps({"id": name, "version": "1.0.0"})
        )
        (d / "install.sh").write_text(f"#!/bin/sh\necho {name} >> /install-order.log\n")
    new_config = {
        **config,
        "features": {
            **config.get("features", {}),
            "./features/alpha": {},
            "./features/beta": {},
        },
    }
    (workspace / ".devcontainer" / "devcontainer.json").write_text(
        json.dumps(new_config)
    )
    return new_config


@given(
    'the config overrides feature install order with "beta" first',
    target_fixture="config",
)
def given_override_beta_first(workspace, config):
    new_config = {
        **config,
        "overrideFeatureInstallOrder": ["./features/beta", "./features/alpha"],
    }
    (workspace / ".devcontainer" / "devcontainer.json").write_text(
        json.dumps(new_config)
    )
    return new_config


@then('the install log shows "beta" installed before "alpha"')
def then_install_log_beta_before_alpha(workspace, config):
    cid = _container_id(workspace, config)
    assert cid, "no running container found"
    result = subprocess.run(
        ["docker", "exec", cid, "cat", "/install-order.log"],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, f"failed to read install log: {result.stderr}"
    lines = result.stdout.strip().splitlines()
    assert "beta" in lines and "alpha" in lines, (
        f"expected both beta and alpha in log, got: {lines}"
    )
    assert lines.index("beta") < lines.index("alpha"), (
        f"expected beta before alpha, got order: {lines}"
    )
