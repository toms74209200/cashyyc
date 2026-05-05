import subprocess
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]


@pytest.fixture(scope="session")
def cyyc_binary():
    subprocess.run(
        ["cargo", "build", "--bin", "cyyc"],
        cwd=REPO_ROOT,
        check=True,
    )
    return REPO_ROOT / "target" / "debug" / "cyyc"


@pytest.fixture
def workspace(tmp_path):
    devcontainer_dir = tmp_path / ".devcontainer"
    devcontainer_dir.mkdir()
    yield tmp_path
    for cfg_path in devcontainer_dir.rglob("devcontainer.json"):
        r = subprocess.run(
            [
                "docker",
                "ps",
                "-aq",
                "--filter",
                f"label=devcontainer.config_file={cfg_path}",
            ],
            capture_output=True,
            text=True,
        )
        ids = r.stdout.split()
        if ids:
            subprocess.run(["docker", "rm", "-f", *ids], capture_output=True)
    subprocess.run(
        [
            "docker",
            "compose",
            "-p",
            f"{tmp_path.name}_devcontainer",
            "down",
            "-v",
            "--remove-orphans",
        ],
        capture_output=True,
    )
    name = tmp_path.name.lower()
    r = subprocess.run(
        ["docker", "images", "-q", "--filter", f"reference=vsc-{name}*"],
        capture_output=True,
        text=True,
    )
    img_ids = list(set(r.stdout.split()))
    if img_ids:
        subprocess.run(["docker", "rmi", "-f", *img_ids], capture_output=True)


@pytest.fixture
def config():
    return {}


@pytest.fixture
def named_configs():
    return {}


@pytest.fixture
def run_result():
    return {}


@pytest.fixture
def container_id_before():
    return [None]


def container_id_by_devcontainer(
    workspace: Path, *, all_states: bool = False
) -> str | None:
    flag = "-aq" if all_states else "-q"
    for cfg_path in (workspace / ".devcontainer").rglob("devcontainer.json"):
        r = subprocess.run(
            [
                "docker",
                "ps",
                flag,
                "--filter",
                f"label=devcontainer.config_file={cfg_path}",
            ],
            capture_output=True,
            text=True,
        )
        out = r.stdout.strip()
        if out:
            return out.split("\n")[0]
    return None


def container_id_by_compose(workspace: Path, *, all_states: bool = False) -> str | None:
    flag = "-aq" if all_states else "-q"
    r = subprocess.run(
        [
            "docker",
            "ps",
            flag,
            "--filter",
            f"label=com.docker.compose.project={workspace.name}_devcontainer",
        ],
        capture_output=True,
        text=True,
    )
    out = r.stdout.strip()
    if out:
        return out.split("\n")[0]
    return None
