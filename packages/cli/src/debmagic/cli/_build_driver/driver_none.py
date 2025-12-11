import os
from pathlib import Path
from typing import Sequence

from debmagic.common.utils import run_cmd, run_cmd_in_foreground

from .common import BuildConfig, BuildDriver


class BuildDriverNone(BuildDriver):
    def __init__(self, config: BuildConfig) -> None:
        self._config = config

    @classmethod
    def create(cls, config: BuildConfig):
        return cls(config=config)

    def run_command(self, cmd: Sequence[str | Path], cwd: Path | None = None, requires_root: bool = False):
        if requires_root and not os.getuid() == 0:
            cmd = ["sudo", *cmd]
        run_cmd(cmd=cmd, dry_run=self._config.dry_run, cwd=cwd)

    def cleanup(self):
        pass

    def drop_into_shell(self):
        run_cmd_in_foreground(["/usr/bin/env", "bash"])
