import json
import os
import re
import subprocess
import time

from pytest_bdd import given, parsers, scenarios, then

from conftest import (
    _container_id,
    container_id_by_devcontainer,
    container_ids_by_compose,
)


def _expand_template(workspace, template):
    if "${devcontainerId}" in template:
        return None
    result = template
    result = result.replace("${localWorkspaceFolder}", str(workspace))
    result = result.replace("${localWorkspaceFolderBasename}", workspace.name)
    result = result.replace(
        "${containerWorkspaceFolder}", f"/workspaces/{workspace.name}"
    )
    result = result.replace("${containerWorkspaceFolderBasename}", workspace.name)
    m = re.search(r"\$\{localEnv:([^}]+)\}", result)
    if m:
        result = (
            result[: m.start()] + os.environ.get(m.group(1), "") + result[m.end() :]
        )
    return result


def _assert_expansion(actual, workspace, template):
    expected = _expand_template(workspace, template)
    if expected is None:
        assert "${" not in actual.strip() and actual.strip(), (
            f"{template!r} not expanded; got {actual!r}"
        )
    else:
        assert os.path.normpath(actual.strip()) == os.path.normpath(expected), (
            f"{template!r}: expected {expected!r}, got {actual.strip()!r}"
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


@then(parsers.parse('the container env "{key}" is the expansion of "{template}"'))
def then_container_env_is_expansion(workspace, config, key, template):
    cid = _container_id(workspace, config)
    assert cid, "no running container found"
    result = subprocess.run(
        ["docker", "exec", cid, "printenv", key],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, f"env {key!r} not set: {result.stderr!r}"
    _assert_expansion(result.stdout, workspace, template)


@then(
    parsers.parse(
        'the file "{path}" in the container contains the expansion of "{template}"'
    )
)
def then_file_in_container_contains_expansion(workspace, config, path, template):
    cid = _container_id(workspace, config)
    assert cid, "no running container found"
    result = subprocess.run(
        ["docker", "exec", cid, "cat", path],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, f"file {path!r} not found: {result.stderr!r}"
    _assert_expansion(result.stdout, workspace, template)


@then(
    parsers.parse(
        'the file "{path}" in the workspace contains the expansion of "{template}"'
    )
)
def then_file_in_workspace_contains_expansion(workspace, path, template):
    full = workspace / path
    assert full.exists(), f"file {path!r} not in workspace"
    _assert_expansion(full.read_text(), workspace, template)


@then(
    parsers.parse(
        'the container has a mount destination matching the expansion of "{template}"'
    )
)
def then_mount_destination_matches_expansion(workspace, config, template):
    cid = _container_id(workspace, config)
    assert cid, "no running container found"
    result = subprocess.run(
        [
            "docker",
            "inspect",
            cid,
            "--format",
            "{{range .Mounts}}{{.Destination}}\n{{end}}",
        ],
        capture_output=True,
        text=True,
    )
    destinations = result.stdout.strip().splitlines()
    expected = _expand_template(workspace, template)
    if expected is None:
        assert not any("${" in d for d in destinations), (
            f"unexpanded variable in mount destinations: {destinations}"
        )
    else:
        assert os.path.normpath(expected) in [
            os.path.normpath(d) for d in destinations
        ], f"mount at {expected!r} not found; found: {destinations}"


@then(
    parsers.parse(
        'the container has a mount source matching the expansion of "{template}"'
    )
)
def then_mount_source_matches_expansion(workspace, config, template):
    cid = _container_id(workspace, config)
    assert cid, "no running container found"
    result = subprocess.run(
        [
            "docker",
            "inspect",
            cid,
            "--format",
            "{{range .Mounts}}{{.Source}}\n{{end}}",
        ],
        capture_output=True,
        text=True,
    )
    sources = result.stdout.strip().splitlines()
    expected = _expand_template(workspace, template)
    if expected is None:
        assert not any("${" in s for s in sources), (
            f"unexpanded variable in mount sources: {sources}"
        )
    else:
        assert os.path.normpath(expected) in [os.path.normpath(s) for s in sources], (
            f"mount source {expected!r} not found; found: {sources}"
        )


@then(parsers.parse('the container user is the expansion of "{template}"'))
def then_container_user_is_expansion(workspace, config, template):
    cid = _container_id(workspace, config)
    assert cid, "no running container found"
    result = subprocess.run(
        ["docker", "inspect", cid, "--format", "{{.Config.User}}"],
        capture_output=True,
        text=True,
    )
    _assert_expansion(result.stdout, workspace, template)


@then("the build log is not in stderr")
def then_build_log_not_in_stderr(run_result):
    assert "FROM" not in run_result["stderr"]


@then("the build log is in stderr")
def then_build_log_in_stderr(run_result):
    assert "FROM" in run_result["stderr"] or "exit 1" in run_result["stderr"], (
        f"expected build log in stderr, got: {run_result['stderr']!r}"
    )


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


@given("the config has a local feature with manifest:", target_fixture="config")
def given_local_feature_with_manifest(workspace, config, docstring):
    manifest = json.loads(docstring)
    feature_id = manifest["id"]
    feature_dir = workspace / ".devcontainer" / "features" / feature_id
    feature_dir.mkdir(parents=True, exist_ok=True)
    (feature_dir / "devcontainer-feature.json").write_text(docstring)
    (feature_dir / "install.sh").write_text("#!/bin/sh\n")
    features = {**config.get("features", {}), f"./features/{feature_id}": {}}
    new_config = {**config, "features": features}
    (workspace / ".devcontainer" / "devcontainer.json").write_text(
        json.dumps(new_config)
    )
    return new_config


@given(
    "the config has a local feature that logs install user variables",
    target_fixture="config",
)
def given_local_feature_logs_install_user_variables(workspace, config):
    feature_id = "logs-install-user-vars"
    feature_dir = workspace / ".devcontainer" / "features" / feature_id
    feature_dir.mkdir(parents=True, exist_ok=True)
    (feature_dir / "devcontainer-feature.json").write_text(
        json.dumps({"id": feature_id, "version": "1.0.0"})
    )
    (feature_dir / "install.sh").write_text(
        "#!/bin/sh\n"
        "{\n"
        '  echo "_CONTAINER_USER=$_CONTAINER_USER"\n'
        '  echo "_REMOTE_USER=$_REMOTE_USER"\n'
        '  echo "_CONTAINER_USER_HOME=$_CONTAINER_USER_HOME"\n'
        '  echo "_REMOTE_USER_HOME=$_REMOTE_USER_HOME"\n'
        "} >> /tmp/feature-user-vars.log\n"
    )
    features = {**config.get("features", {}), f"./features/{feature_id}": {}}
    new_config = {**config, "features": features}
    (workspace / ".devcontainer" / "devcontainer.json").write_text(
        json.dumps(new_config)
    )
    return new_config


@then("the container runs in privileged mode")
def then_container_privileged(workspace, config):
    cid = _container_id(workspace, config)
    assert cid, "no running container found"
    result = subprocess.run(
        ["docker", "inspect", cid, "--format", "{{.HostConfig.Privileged}}"],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, f"docker inspect failed: {result.stderr}"
    assert result.stdout.strip() == "true", (
        f"expected container to be privileged, got: {result.stdout.strip()!r}"
    )


@then("the container runs with init process")
def then_container_init(workspace, config):
    cid = _container_id(workspace, config)
    assert cid, "no running container found"
    result = subprocess.run(
        ["docker", "inspect", cid, "--format", "{{.HostConfig.Init}}"],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, f"docker inspect failed: {result.stderr}"
    assert result.stdout.strip() == "true", (
        f"expected container to have init process, got: {result.stdout.strip()!r}"
    )


@then(parsers.parse('the container has capability "{cap}"'))
def then_container_has_capability(workspace, config, cap):
    cid = _container_id(workspace, config)
    assert cid, "no running container found"
    result = subprocess.run(
        ["docker", "inspect", cid, "--format", "{{.HostConfig.CapAdd}}"],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, f"docker inspect failed: {result.stderr}"
    assert cap in result.stdout, (
        f"expected capability {cap!r} in CapAdd, got: {result.stdout.strip()!r}"
    )


@then(parsers.parse('the container has security option "{opt}"'))
def then_container_has_security_option(workspace, config, opt):
    cid = _container_id(workspace, config)
    assert cid, "no running container found"
    result = subprocess.run(
        ["docker", "inspect", cid, "--format", "{{.HostConfig.SecurityOpt}}"],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, f"docker inspect failed: {result.stderr}"
    assert opt in result.stdout, (
        f"expected security option {opt!r} in SecurityOpt, got: {result.stdout.strip()!r}"
    )


@then(parsers.parse('the container image entrypoint includes "{path}"'))
def then_image_entrypoint_includes(workspace, config, path):
    cid = _container_id(workspace, config)
    assert cid, "no running container found"
    image_id_result = subprocess.run(
        ["docker", "inspect", cid, "--format", "{{.Image}}"],
        capture_output=True,
        text=True,
    )
    assert image_id_result.returncode == 0, (
        f"docker inspect failed: {image_id_result.stderr}"
    )
    image_id = image_id_result.stdout.strip()
    result = subprocess.run(
        [
            "docker",
            "image",
            "inspect",
            image_id,
            "--format",
            "{{json .Config.Entrypoint}}",
        ],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, f"docker image inspect failed: {result.stderr}"
    entrypoint = json.loads(result.stdout.strip())
    assert path in entrypoint, (
        f"expected {path!r} in image entrypoint, got: {entrypoint!r}"
    )
