import json
import re
import subprocess

from pytest_bdd import given, parsers, scenarios, then

scenarios("../../features/ps.feature")


@given(
    parsers.parse('the compose file also defines a runService "{service}"'),
    target_fixture="config",
)
def given_compose_run_service(workspace, config, service):
    compose_path = workspace / ".devcontainer" / "docker-compose.yml"
    existing = compose_path.read_text()
    m = re.search(r"image:\s+(\S+)", existing)
    image = m.group(1) if m else "mcr.microsoft.com/devcontainers/base:debian"
    service_yaml = f"  {service}:\n    image: {image}\n    command: sleep infinity\n"
    compose_path.write_text(existing + service_yaml)
    new_config = {**config, "runServices": [service]}
    (workspace / ".devcontainer" / "devcontainer.json").write_text(
        json.dumps(new_config)
    )
    return new_config


@given(parsers.parse('a running container exists for the "{name}" config'))
def given_running_container_for_named(workspace, named_configs, name, cyyc_binary):
    subprocess.run(
        [str(cyyc_binary), "shell", name],
        cwd=workspace,
        stdin=subprocess.DEVNULL,
        capture_output=True,
        text=True,
        timeout=600,
    )
    cfg_path = workspace / ".devcontainer" / name / "devcontainer.json"
    r = subprocess.run(
        [
            "docker",
            "ps",
            "-q",
            "--filter",
            f"label=devcontainer.config_file={cfg_path}",
        ],
        capture_output=True,
        text=True,
    )
    cid = r.stdout.strip().split("\n")[0] if r.stdout.strip() else None
    assert cid, f"precondition failed: container for '{name}' config is not running"


@given(parsers.parse('no container exists for the "{name}" config'))
def given_no_container_for_named(name):
    pass


@then(parsers.parse('the listing shows the config with status "{status}"'))
def then_listing_shows_status(run_result, status):
    output = run_result["stdout"]
    assert status in output, f"expected {status!r} in output:\n{output!r}"


@then("the container ID is printed")
def then_container_id_printed(run_result, container_id_before):
    cid = container_id_before[0]
    assert cid, "no container ID recorded"
    short_id = cid[:12]
    output = run_result["stdout"]
    assert short_id in output, (
        f"expected container ID {short_id!r} in output:\n{output!r}"
    )


@then(parsers.parse('the listing shows "{name}" with status "{status}"'))
def then_listing_shows_named_status(run_result, name, status):
    output = run_result["stdout"]
    for line in output.splitlines():
        if name in line and status in line:
            return
    raise AssertionError(f"expected line with {name!r} and {status!r} in:\n{output!r}")


@then(parsers.parse('the "{service}" service is not listed'))
def then_service_not_listed(run_result, service):
    output = run_result["stdout"]
    assert service not in output, (
        f"expected {service!r} not to appear in output:\n{output!r}"
    )
