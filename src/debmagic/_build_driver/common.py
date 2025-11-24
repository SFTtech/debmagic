import abc
from dataclasses import dataclass
from pathlib import Path
from typing import Literal, Self, Sequence

BuildDriverType = Literal["docker"] | Literal["lxd"] | Literal["none"]
SUPPORTED_BUILD_DRIVERS: list[BuildDriverType] = ["docker", "none"]


class BuildError(RuntimeError):
    pass


@dataclass
class BuildConfig:
    package_identifier: str
    source_dir: Path
    output_dir: Path
    dry_run: bool
    distro_version: str  # e.g. trixie
    distro: str  # e.g. debian
    sign_package: bool  # TODO: figure out if this is the right place

    # build paths
    build_root_dir: Path

    @property
    def build_work_dir(self) -> Path:
        return self.build_root_dir / "work"

    @property
    def build_temp_dir(self) -> Path:
        return self.build_root_dir / "temp"

    @property
    def build_source_dir(self) -> Path:
        return self.build_work_dir / self.package_identifier

    def create_dirs(self):
        self.output_dir.mkdir(exist_ok=True, parents=True)
        self.build_work_dir.mkdir(exist_ok=True, parents=True)
        self.build_temp_dir.mkdir(exist_ok=True, parents=True)
        self.build_source_dir.mkdir(exist_ok=True, parents=True)


class BuildDriver:
    @classmethod
    @abc.abstractmethod
    def create(cls, config: BuildConfig) -> Self:
        pass

    @abc.abstractmethod
    def run_command(self, args: Sequence[str | Path], cwd: Path | None = None, requires_root: bool = False):
        pass

    @abc.abstractmethod
    def cleanup(self):
        pass

    @abc.abstractmethod
    def drop_into_shell(self):
        pass
