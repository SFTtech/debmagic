import uuid
from pathlib import Path
from typing import Self, Sequence

from debmagic._build_driver.common import BuildConfig, BuildDriver, BuildError
from debmagic._utils import run_cmd, run_cmd_in_foreground

BUILD_DIR_IN_CONTAINER = Path("/debmagic")

DOCKERFILE_TEMPLATE = f"""
FROM docker.io/{{distro}}:{{distro_version}}

RUN apt-get update && apt-get -y install dpkg-dev

RUN mkdir -p {BUILD_DIR_IN_CONTAINER}
ENTRYPOINT ["sleep", "infinity"]
"""


class BuildDriverDocker(BuildDriver):
    def __init__(self, config: BuildConfig, container_name: str):
        self._config = config

        self._container_name = container_name

    def _translate_path_in_container(self, path_in_source: Path) -> Path:
        if not path_in_source.is_relative_to(self._config.build_root_dir):
            raise BuildError("Cannot run in a path not relative to the original source directory")
        rel = path_in_source.relative_to(self._config.build_root_dir)
        return BUILD_DIR_IN_CONTAINER / rel

    @classmethod
    def create(cls, config: BuildConfig) -> Self:
        formatted_dockerfile = DOCKERFILE_TEMPLATE.format(
            distro=config.distro,
            distro_version=config.distro_version,
        )

        dockerfile_path = config.build_temp_dir / "Dockerfile"
        dockerfile_path.write_text(formatted_dockerfile)

        docker_image_name = str(uuid.uuid4())
        ret = run_cmd(
            [
                "docker",
                "build",
                "--tag",
                docker_image_name,
                "-f",
                dockerfile_path,
                config.build_temp_dir,
            ],
            dry_run=config.dry_run,
            check=False,
        )
        if ret.returncode != 0:
            raise BuildError("Error creating docker image for build")

        docker_container_name = str(uuid.uuid4())
        ret = run_cmd(
            [
                "docker",
                "run",
                "--detach",
                "--name",
                docker_container_name,
                "--mount",
                f"type=bind,src={config.build_root_dir},dst={BUILD_DIR_IN_CONTAINER}",
                docker_image_name,
            ],
            dry_run=config.dry_run,
            check=False,
        )
        if ret.returncode != 0:
            raise BuildError("Error creating docker image for build")

        instance = cls(config=config, container_name=docker_container_name)
        return instance

    def run_command(self, cmd: Sequence[str | Path], cwd: Path | None = None, requires_root: bool = False):
        del requires_root  # we assume to always be root in the container

        if cwd:
            cwd = self._translate_path_in_container(cwd)
            cwd_args: list[str | Path] = ["--workdir", cwd]
        else:
            cwd_args = []

        ret = run_cmd(["docker", "exec", *cwd_args, self._container_name, *cmd], dry_run=self._config.dry_run)
        if ret.returncode != 0:
            raise BuildError("Error building package")

    def cleanup(self):
        run_cmd(["docker", "rm", "-f", self._container_name], dry_run=self._config.dry_run)

    def drop_into_shell(self):
        if not self._config.dry_run:
            run_cmd_in_foreground(
                ["docker", "exec", "--interactive", "--tty", self._container_name, "/usr/bin/env", "bash"]
            )
