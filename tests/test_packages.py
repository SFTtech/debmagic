import subprocess
import shlex
from pathlib import Path

import pytest

PACKAGES_BASE_PATH = Path(__file__).parent / "packages"

UBUNTU_RELEASE = "ubuntu/noble"  # use 24.04 for testing

def run(cmd: list[str], cwd: Path | None = None, check: bool = False):
    print(f"$ {shlex.join(str(p) for p in cmd)}")
    subprocess.run(cmd, cwd=cwd, check=check)


def clone_repo(package_name: str, repo_url: str):
    package_path = PACKAGES_BASE_PATH / package_name
    dest_path = package_path / "pkg_build"
    dest_path_repo = dest_path / "upstream_src"
    if not dest_path_repo.exists():
        subprocess.check_call(["git", "clone", "-b", UBUNTU_RELEASE, "--depth=1", repo_url, dest_path_repo])

    target_rules = dest_path_repo / "debian" / "rules"
    rules = (package_path / "rules.py").relative_to(target_rules.parent, walk_up=True)
    target_rules.unlink()
    target_rules.symlink_to(rules)
    return dest_path_repo


package_fixtures = [("htop", "https://git.launchpad.net/ubuntu/+source/htop")]


@pytest.mark.parametrize(
    "package, src_url", package_fixtures, ids=map(lambda x: x[0], package_fixtures)
)
def test_build_package(package: str, src_url: str):
    repo_dir = clone_repo(package_name=package, repo_url=src_url)
    run(["sudo", "apt-get", "-y", "build-dep", repo_dir], check=True)
    run(["debuild", "-nc", "-uc", "-b"], cwd=repo_dir, check=True)
