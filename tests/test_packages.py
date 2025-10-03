import shutil
import subprocess
from pathlib import Path
import pytest

PACKAGES_BASE_PATH = Path(__file__).parent / "packages"


def clone_repo(package_name: str, repo_url: str):
    package_path = PACKAGES_BASE_PATH / package_name
    dest_path = package_path / "pkg_build"
    dest_path_repo = dest_path / "upstream_src"
    if not dest_path_repo.exists():
        subprocess.check_call(["git", "clone", repo_url, dest_path_repo])
        subprocess.check_call(
            ["git", "checkout", "ubuntu/noble-devel"], cwd=dest_path_repo
        )
    shutil.copy(package_path / "rules.py", dest_path_repo / "debian" / "rules")
    return dest_path_repo


package_fixtures = [("htop", "https://git.launchpad.net/ubuntu/+source/htop")]


@pytest.mark.parametrize(
    "package, src_url", package_fixtures, ids=map(lambda x: x[0], package_fixtures)
)
def test_build_package(package: str, src_url: str):
    repo_dir = clone_repo(package_name=package, repo_url=src_url)
    subprocess.run(["sudo", "apt-get", "-y", "build-dep", repo_dir], check=True)
    subprocess.run(["debuild", "-nc", "-uc", "-b"], cwd=repo_dir, check=True)
