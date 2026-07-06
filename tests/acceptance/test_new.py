import json
import subprocess

import pexpect
from pytest_bdd import given, parsers, scenarios, then, when

scenarios("../../features/new.feature")


@when(
    parsers.parse(
        'running "cyyc new" and selecting the first template and confirming features'
    )
)
def when_running_new_interactive(workspace, cyyc_binary, run_result):
    child = pexpect.spawn(
        str(cyyc_binary),
        ["new"],
        cwd=str(workspace),
        timeout=120,
    )
    child.expect("Template:")
    child.sendline("")
    child.expect("Features:")
    child.sendline("")
    child.expect(pexpect.EOF)
    child.close()
    run_result.update(
        {
            "stdout": child.before.decode("utf-8", errors="replace")
            if child.before
            else "",
            "stderr": "",
            "returncode": child.exitstatus or 0,
        }
    )


@then(".devcontainer/devcontainer.json is created with a valid template")
def then_devcontainer_json_created(workspace):
    path = workspace / ".devcontainer" / "devcontainer.json"
    assert path.exists(), f"{path} was not created"
    config = json.loads(path.read_text())
    assert "image" in config or "dockerFile" in config, (
        f"expected 'image' or 'dockerFile' key, got: {list(config.keys())}"
    )
