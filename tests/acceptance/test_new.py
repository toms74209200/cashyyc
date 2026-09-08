import json
import re

import pexpect
from pytest_bdd import parsers, scenarios, then, when

scenarios("../../features/new.feature")

# A prompt starts by hiding the cursor and clearing the label line, which
# separates a new prompt from the redraws a prompt emits on every keystroke.
_PROMPT = re.compile(rb"\x1b\[\?25l\x1b\[2K\r([A-Za-z0-9_]+): ")


def _next_prompt(child):
    child.expect(_PROMPT)
    return child.match.group(1).decode()


def _accept_default_options(child):
    """Answer every template option prompt until the feature prompt shows up."""
    while _next_prompt(child) != "Features":
        child.sendline("")


def _finish(child, run_result):
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


@when(
    parsers.parse(
        'running "cyyc new" and selecting the "{template}" template '
        'with the default options and the "{feature}" feature'
    )
)
def when_running_new_with_template_and_feature(
    workspace, cyyc_binary, run_result, template, feature
):
    child = pexpect.spawn(
        str(cyyc_binary),
        ["new"],
        cwd=str(workspace),
        timeout=120,
    )
    assert _next_prompt(child) == "Template"
    child.send(template)
    child.sendline("")
    _accept_default_options(child)
    child.send(feature)
    child.send(" ")
    child.sendline("")
    _finish(child, run_result)


@then(".devcontainer/devcontainer.json is created with a valid template")
def then_devcontainer_json_created(workspace):
    path = workspace / ".devcontainer" / "devcontainer.json"
    assert path.exists(), f"{path} was not created"
    config = json.loads(path.read_text())
    assert "image" in config or "dockerFile" in config, (
        f"expected 'image' or 'dockerFile' key, got: {list(config.keys())}"
    )


@then("no template option placeholder remains in .devcontainer/devcontainer.json")
def then_no_template_option_placeholder(workspace):
    content = (workspace / ".devcontainer" / "devcontainer.json").read_text()
    assert "${templateOption:" not in content, (
        f"unresolved template option in: {content}"
    )


@then(
    parsers.parse('.devcontainer/devcontainer.json declares the feature "{feature_id}"')
)
def then_declares_feature(workspace, feature_id):
    config = json.loads((workspace / ".devcontainer" / "devcontainer.json").read_text())
    features = config.get("features") or {}
    assert feature_id in features, (
        f"expected feature {feature_id}, got: {list(features)}"
    )
