import re

import json5
import pexpect
import pytest
from pytest_bdd import given, parsers, scenarios, then, when

scenarios("../../features/new.feature")

# A prompt starts by hiding the cursor and clearing the label line, which
# separates a new prompt from the redraws a prompt emits on every keystroke.
_PROMPT = re.compile(rb"\x1b\[\?25l\x1b\[2K\r([A-Za-z0-9_ ]+): ")


@pytest.fixture
def answers():
    return {"template": None, "features": []}


@given(parsers.parse('the "{template}" template is selected'))
def given_template_selected(answers, template):
    answers["template"] = template


@given("every template option takes its default")
def given_default_options():
    pass


@given(parsers.parse('the "{feature}" feature is selected'))
def given_feature_selected(answers, feature):
    answers["features"].append(feature)


@given("no feature is selected")
def given_no_feature(answers):
    answers["features"] = []


@given(parsers.parse('the file "{path}" already exists'))
def given_file_already_exists(workspace, path):
    target = workspace / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text("placed by the test\n")


@when('running "cyyc new"')
def when_running_new(workspace, cyyc_binary, run_result, answers):
    child = pexpect.spawn(
        str(cyyc_binary),
        ["new"],
        cwd=str(workspace),
        timeout=120,
    )
    while True:
        try:
            child.expect(_PROMPT)
        except pexpect.EOF:
            break
        label = child.match.group(1).decode()
        if label == "Template":
            child.send(answers["template"])
        elif label == "Features":
            for feature in answers["features"]:
                child.send(feature)
                child.send(" ")
        # An optional-file prompt selects nothing, and a template option
        # offers its default first, so a bare Enter answers both.
        child.sendline("")
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
    config = json5.loads(path.read_text())
    assert "image" in config or "dockerFile" in config, (
        f"expected 'image' or 'dockerFile' key, got: {list(config.keys())}"
    )


@then(parsers.parse("no template option placeholder remains in {path}"))
def then_no_template_option_placeholder(workspace, path):
    content = (workspace / path).read_text()
    assert "${templateOption:" not in content, (
        f"unresolved template option in {path}: {content}"
    )


@then(parsers.parse("{path} keeps the comments the template ships"))
def then_keeps_template_comments(workspace, path):
    content = (workspace / path).read_text()
    assert re.search(r"^\s*//", content, re.MULTILINE), (
        f"no comment left in {path}: {content}"
    )


@then(parsers.parse("{path} is created"))
def then_file_created(workspace, path):
    assert (workspace / path).exists(), f"{path} was not created"


@then(parsers.parse("{path} is not created"))
def then_file_not_created(workspace, path):
    assert not (workspace / path).exists(), f"{path} was created"


@then(parsers.parse("{path} is executable"))
def then_file_executable(workspace, path):
    target = workspace / path
    assert target.exists(), f"{path} was not created"
    assert target.stat().st_mode & 0o111, f"{path} is not executable"


@then(
    parsers.parse('.devcontainer/devcontainer.json declares the feature "{feature_id}"')
)
def then_declares_feature(workspace, feature_id):
    config = json5.loads(
        (workspace / ".devcontainer" / "devcontainer.json").read_text()
    )
    features = config.get("features") or {}
    assert feature_id in features, (
        f"expected feature {feature_id}, got: {list(features)}"
    )
