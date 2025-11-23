import abc
from dataclasses import dataclass
from pathlib import Path
from typing import Literal, Sequence

BuildDriverType = Literal["docker"] | Literal["lxd"] | Literal["none"]
SUPPORTED_BUILD_DRIVERS: list[BuildDriverType] = ["docker", "none"]


class BuildError(RuntimeError):
    pass


@dataclass
class BuildConfig:
    source_dir: Path
    output_dir: Path
    dry_run: bool
    distro_version: str  # e.g. trixie
    distro: str = "debian"


class BuildDriver:
    @classmethod
    @abc.abstractmethod
    def create(cls, config: BuildConfig) -> "BuildDriver":
        pass

    @abc.abstractmethod
    def run_command(self, args: Sequence[str | Path], cwd: Path | None = None, requires_root: bool = False):
        pass

    @abc.abstractmethod
    def copy_file(self, source_dir: Path, glob: str, dest_dir: Path):
        pass

    @abc.abstractmethod
    def cleanup(self):
        pass

    @abc.abstractmethod
    def drop_into_shell(self):
        pass
