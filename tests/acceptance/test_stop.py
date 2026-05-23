import subprocess

from pytest_bdd import scenarios, then

from conftest import (
    container_id_by_devcontainer,
    container_ids_by_compose,
)

scenarios("../../features/stop.feature")


def _container_state(container_id: str) -> str:
    r = subprocess.run(
        ["docker", "inspect", "--format", "{{.State.Status}}", container_id],
        capture_output=True,
        text=True,
    )
    return r.stdout.strip()


@then("the container is stopped")
def then_container_stopped(workspace, config, container_id_before):
    if "dockerComposeFile" in config:
        ids = container_ids_by_compose(workspace, all_states=True)
        cid = ids[0] if ids else None
    else:
        cid = container_id_by_devcontainer(workspace, all_states=True)
    assert cid, "no container found"
    assert cid == container_id_before[0], (
        f"expected same container {container_id_before[0]}, got {cid}"
    )
    state = _container_state(cid)
    assert state in ("exited", "created"), f"expected stopped state, got {state!r}"


@then("the container is not removed")
def then_container_not_removed(workspace, config, container_id_before):
    if "dockerComposeFile" in config:
        ids = container_ids_by_compose(workspace, all_states=True)
        cid = ids[0] if ids else None
    else:
        cid = container_id_by_devcontainer(workspace, all_states=True)
    assert cid == container_id_before[0], (
        f"expected container {container_id_before[0]} to still exist, got {cid}"
    )


@then("all compose containers are stopped")
def then_all_compose_containers_stopped(workspace):
    cids = container_ids_by_compose(workspace, all_states=True)
    assert len(cids) >= 2, f"expected at least 2 containers, got {cids}"
    for cid in cids:
        state = _container_state(cid)
        assert state in ("exited", "created"), (
            f"expected container {cid} to be stopped, got {state!r}"
        )


@then("all compose containers are not removed")
def then_all_compose_containers_not_removed(workspace):
    cids = container_ids_by_compose(workspace, all_states=True)
    assert len(cids) >= 2, f"expected at least 2 containers to still exist, got {cids}"
