from pathlib import Path
from typing import Self, Sequence

from pydantic import BaseModel

from .common import BuildConfig, BuildDriver


class LxdDriverConfig(BaseModel):
    pass


class BuildDriverLxd(BuildDriver[LxdDriverConfig]):
    @classmethod
    def create(cls, config: BuildConfig, driver_config: LxdDriverConfig) -> Self:
        raise NotImplementedError()

    @classmethod
    def from_build_root(cls, build_root: Path) -> Self:
        raise NotImplementedError()

    def run_command(self, cmd: Sequence[str | Path], cwd: Path | None = None, requires_root: bool = False):
        raise NotImplementedError()

    def cleanup(self):
        raise NotImplementedError()

    def drop_into_shell(self):
        raise NotImplementedError()
