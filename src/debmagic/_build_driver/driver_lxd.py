from pathlib import Path
from typing import Sequence

from debmagic._build_driver.common import BuildConfig, BuildDriver


class BuildDriverLxd(BuildDriver):
    @classmethod
    def create(cls, config: BuildConfig):
        return cls()

    def run_command(self, args: Sequence[str | Path], cwd: Path | None = None, requires_root: bool = False):
        raise NotImplementedError()

    def cleanup(self):
        raise NotImplementedError()

    def drop_into_shell(self):
        raise NotImplementedError()
