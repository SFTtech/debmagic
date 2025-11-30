import abc
from dataclasses import dataclass
from pathlib import Path
from typing import Literal, Self, Sequence

from pydantic import BaseModel

BuildDriverType = Literal["docker"] | Literal["lxd"] | Literal["none"]
SUPPORTED_BUILD_DRIVERS: list[BuildDriverType] = ["docker", "none"]


class BuildError(RuntimeError):
    pass


@dataclass
class PackageDescription:
    name: str
    version: str
    source_dir: Path


DriverSpecificBuildMetadata = dict[str, str]  # expand as needed


class BuildMetadata(BaseModel):
    driver: BuildDriverType
    build_root: Path
    source_dir: Path
    driver_metadata: DriverSpecificBuildMetadata


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
    def build_identifier(self) -> str:
        # TODO: include distro + distro version + architecture
        return self.package_identifier

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

    @classmethod
    @abc.abstractmethod
    def from_build_metadata(cls, build_metadata: BuildMetadata) -> Self:
        pass

    @abc.abstractmethod
    def get_build_metadata(self) -> DriverSpecificBuildMetadata:
        pass

    @abc.abstractmethod
    def run_command(self, cmd: Sequence[str | Path], cwd: Path | None = None, requires_root: bool = False):
        pass

    @abc.abstractmethod
    def cleanup(self):
        pass

    @abc.abstractmethod
    def drop_into_shell(self):
        pass

    @abc.abstractmethod
    def driver_type(self) -> BuildDriverType:
        pass
