import shlex
import subprocess
from pathlib import Path

import pytest

PACKAGES_BASE_PATH = Path(__file__).parent / "packages"


def run(cmd: list[str], cwd: Path | None = None, check: bool = False):
    print(f"$ {shlex.join(str(p) for p in cmd)}")
    subprocess.run(cmd, cwd=cwd, check=check)


def fetch_sources(package_name: str, version: str) -> Path:
    package_path = PACKAGES_BASE_PATH / package_name
    dest_path = package_path / "pkg_build"
    dest_path.mkdir(exist_ok=True, parents=True)
    dest_path_repo = dest_path / f"{package_name}-{version}"
    if not dest_path_repo.exists():
        subprocess.check_call(["apt-get", "source", package_name], cwd=dest_path)
    # else: one fetch per day?

    target_rules = dest_path_repo / "debian" / "rules"
    rules = (package_path / "rules.py").relative_to(target_rules.parent, walk_up=True)
    target_rules.unlink()
    target_rules.symlink_to(rules)
    return dest_path_repo


package_fixtures = [
    ("htop", "3.4.1"),
]


@pytest.mark.parametrize("package, version", package_fixtures, ids=[x[0] for x in package_fixtures])
def test_build_package(package: str, version: str):
    # TODO use sandbox/container, lxd?
    repo_dir = fetch_sources(package_name=package, version=version)
    # TODO don't install every time...
    run(["sudo", "apt-get", "-y", "build-dep", str(repo_dir)], check=True)
    run(["debuild", "-nc", "-uc", "-b"], cwd=repo_dir, check=True)

    deb_file = next(repo_dir.parent.glob(f"{package}_{version}*.deb"))
    assert deb_file is not None and deb_file.is_file(), "The resulting .deb archive was created correctly"
