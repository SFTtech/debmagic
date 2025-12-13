import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Generator

import pytest
from debmagic.common.utils import run_cmd

PACKAGES_BASE_PATH = Path(__file__).parent / "packages"

DOCKERFILE_TEMPLATE = """
FROM docker.io/{distro}:{distro_version}

RUN apt-get update && apt-get -y install dpkg-dev python3 python3-pip python3-pydantic
RUN --mount=from=dist,target=/tmp/dist python3 -m pip install --break-system-packages /tmp/dist/debmagic_common-*.whl
RUN --mount=from=dist,target=/tmp/dist python3 -m pip install --break-system-packages /tmp/dist/debmagic_pkg*.whl
"""


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


def _prepare_docker_image(test_tmp_dir: Path, distro: str, distro_version: str):
    debmagic_repo_root_dir = Path(__file__).parent.parent.parent
    run_cmd(["uv", "build", "--package", "debmagic-api"], check=True)
    run_cmd(["uv", "build", "--package", "debmagic"], check=True)

    formatted_dockerfile = DOCKERFILE_TEMPLATE.format(
        distro=distro,
        distro_version=distro_version,
    )

    dockerfile_path = test_tmp_dir / "Dockerfile"
    dockerfile_path.write_text(formatted_dockerfile)

    docker_image_name = f"debmagic-integration-{distro}-{distro_version}"
    run_cmd(
        [
            "docker",
            "build",
            "--tag",
            docker_image_name,
            "--build-context",
            f"dist={debmagic_repo_root_dir / 'dist'}",
            "-f",
            dockerfile_path,
            test_tmp_dir,
        ],
        check=True,
    )

    return docker_image_name


@dataclass
class Environment:
    tmp_dir: Path
    docker_image_name: str


@pytest.fixture(scope="session")
def test_env() -> Generator[Environment]:
    with tempfile.TemporaryDirectory() as test_tmp_dir:
        test_dir = Path(test_tmp_dir)
        image_name = _prepare_docker_image(test_dir, "debian", "trixie")
        yield Environment(tmp_dir=test_dir, docker_image_name=image_name)


package_fixtures = [
    ("htop", "3.4.1"),
]


@pytest.mark.parametrize("package, version", package_fixtures, ids=[x[0] for x in package_fixtures])
def test_build_package(test_env: Environment, package: str, version: str):
    repo_dir = fetch_sources(package_name=package, version=version)

    with tempfile.TemporaryDirectory() as output_dir:
        subprocess.run(
            [
                "uv",
                "run",
                "debmagic",
                "build",
                "--driver",
                "docker",
                "--docker-image",
                test_env.docker_image_name,
                "--source-dir",
                str(repo_dir),
                "--output-dir",
                output_dir,
            ],
            check=False,
        )

        deb_file = next(Path(output_dir).glob(f"{package}_{version}*.deb"))
        assert deb_file is not None and deb_file.is_file(), "The resulting .deb archive was created correctly"
