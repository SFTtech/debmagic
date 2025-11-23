import tempfile
import uuid
from pathlib import Path
from typing import Sequence

from debmagic._build_driver.common import BuildConfig, BuildDriver, BuildError
from debmagic._utils import run_cmd, run_cmd_in_foreground

WORKDIR_IN_CONTAINER = Path("/work/source")
OUTPUT_DIR_IN_CONTAINER = Path("/output")

DOCKERFILE_TEMPLATE = f"""
FROM docker.io/{{distro}}:{{distro_version}}

RUN apt-get update && apt-get -y install devscripts

RUN mkdir -p {WORKDIR_IN_CONTAINER} {OUTPUT_DIR_IN_CONTAINER}
WORKDIR {WORKDIR_IN_CONTAINER}
COPY --from=source_dir . {WORKDIR_IN_CONTAINER}
ENTRYPOINT ["sleep", "infinity"]
"""


class BuildDriverDocker(BuildDriver):
    def __init__(self, workdir: tempfile.TemporaryDirectory, config: BuildConfig, container_name: str):
        self._workdir = workdir
        self._workdir_path = Path(workdir.name)
        self._config = config

        self._container_name = container_name

    def _translate_source_path(self, path_in_source: Path) -> Path:
        if not path_in_source.is_relative_to(self._config.source_dir):
            raise BuildError("Cannot run in a path not relative to the original source directory")
        rel = path_in_source.relative_to(self._config.source_dir)
        return WORKDIR_IN_CONTAINER / rel

    def _translate_output_path(self, path_in_output: Path) -> Path:
        if not path_in_output.is_relative_to(self._config.output_dir):
            raise BuildError("Cannot run in a path not relative to the original output directory")
        rel = path_in_output.relative_to(self._config.output_dir)
        return OUTPUT_DIR_IN_CONTAINER / rel

    @classmethod
    def create(cls, config: BuildConfig):
        workdir = tempfile.TemporaryDirectory()
        workdir_path = Path(workdir.name)

        formatted_dockerfile = DOCKERFILE_TEMPLATE.format(
            distro=config.distro,
            distro_version=config.distro_version,
        )

        dockerfile_path = workdir_path / "Dockerfile"
        dockerfile_path.write_text(formatted_dockerfile)
        gitignore_file = config.source_dir / ".gitignore"
        if gitignore_file.is_file():
            (workdir_path / ".dockerignore").write_text(gitignore_file.read_text())

        docker_image_name = str(uuid.uuid4())
        ret = run_cmd(
            [
                "docker",
                "build",
                "--tag",
                docker_image_name,
                "-f",
                dockerfile_path,
                "--build-context",
                f"source_dir={config.source_dir}",
                workdir_path,
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
                f"type=bind,src={config.output_dir},dst={OUTPUT_DIR_IN_CONTAINER}",
                docker_image_name,
            ],
            dry_run=config.dry_run,
            check=False,
        )
        if ret.returncode != 0:
            raise BuildError("Error creating docker image for build")

        instance = cls(config=config, workdir=workdir, container_name=docker_container_name)
        return instance

    def run_command(self, args: Sequence[str | Path], cwd: Path | None = None, requires_root: bool = False):
        del requires_root  # we assume to always be root in the container

        if cwd:
            cwd = self._translate_source_path(cwd)

        ret = run_cmd(["docker", "exec", self._container_name, *args], dry_run=self._config.dry_run)
        if ret.returncode != 0:
            raise BuildError("Error building package")

    def copy_file(self, source_dir: Path, glob: str, dest_dir: Path):
        translated_source = self._translate_source_path(source_dir)
        translated_output = self._translate_output_path(dest_dir)
        self.run_command(["/usr/bin/env", "bash", "-c", f"cp -f {translated_source}/{glob} {translated_output}"])

    def cleanup(self):
        run_cmd(["docker", "rm", "-f", self._container_name], dry_run=self._config.dry_run)
        self._workdir.cleanup()

    def drop_into_shell(self):
        if not self._config.dry_run:
            run_cmd_in_foreground(
                ["docker", "exec", "--interactive", "--tty", self._container_name, "/usr/bin/env", "bash"]
            )
