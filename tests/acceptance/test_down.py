from pytest_bdd import scenarios, then

from conftest import (
    container_id_by_devcontainer,
    container_ids_by_compose,
)

scenarios("../../features/down.feature")


@then("the container is removed")
def then_container_removed(workspace, config):
    if "dockerComposeFile" in config:
        ids = container_ids_by_compose(workspace, all_states=True)
        cid = ids[0] if ids else None
    else:
        cid = container_id_by_devcontainer(workspace, all_states=True)
    assert cid is None, f"expected container to be removed, but found {cid}"
