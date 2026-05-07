import subprocess

from pytest_bdd import scenarios, then

from conftest import (
    container_id_by_compose,
    container_id_by_devcontainer,
)

scenarios("../../features/down.feature")


@then("the container is removed")
def then_container_removed(workspace, config):
    if "dockerComposeFile" in config:
        cid = container_id_by_compose(workspace, all_states=True)
    else:
        cid = container_id_by_devcontainer(workspace, all_states=True)
    assert cid is None, f"expected container to be removed, but found {cid}"
