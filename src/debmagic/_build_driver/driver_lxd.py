from pathlib import Path
from typing import Sequence

from debmagic._build_driver.common import BuildConfig, BuildDriver


class BuildDriverLxd(BuildDriver):
    @classmethod
    def create(cls, config: BuildConfig):
        return cls()

    def run_command(self, args: Sequence[str | Path], cwd: Path | None = None):
        raise NotImplementedError()

    def cleanup(self):
        pass
