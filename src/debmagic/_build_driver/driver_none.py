import shutil
from pathlib import Path
from typing import Sequence

from debmagic._build_driver.common import BuildConfig, BuildDriver
from debmagic._utils import run_cmd, run_cmd_in_foreground


class BuildDriverNone(BuildDriver):
    def __init__(self, config: BuildConfig) -> None:
        self._config = config

    @classmethod
    def create(cls, config: BuildConfig):
        return cls(config=config)

    def run_command(self, args: Sequence[str | Path], cwd: Path | None = None):
        run_cmd(args=args, dry_run=self._config.dry_run, cwd=cwd)

    def copy_file(self, source_dir: Path, glob: str, dest_dir: Path):
        for file in source_dir.glob(glob):
            shutil.copy(file, dest_dir)

    def cleanup(self):
        pass

    def drop_into_shell(self):
        run_cmd_in_foreground(["/usr/bin/env", "bash"])
