from pathlib import Path
from typing import Self, Sequence

from debmagic._build_driver.common import BuildConfig, BuildDriver


class BuildDriverLxd(BuildDriver):
    @classmethod
    def create(cls, config: BuildConfig) -> Self:
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
