from __future__ import annotations

import shlex
import typing
from dataclasses import dataclass, field
from pathlib import Path

from ._build_stage import BuildStage
from ._utils import run_cmd

if typing.TYPE_CHECKING:
    from ._package import BinaryPackage, PackageFilter, SourcePackage
    from ._preset import Preset
    from ._utils import Namespace


@dataclass
class Build:
    presets: list[Preset]
    source_package: SourcePackage
    source_dir: Path
    binary_packages: list[BinaryPackage]
    install_base_dir: Path
    architecture_target: str
    architecture_host: str
    flags: Namespace
    parallel: int
    prefix: Path
    dry_run: bool = False

    _completed_stages: set[BuildStage] = field(default_factory=set)
    _selected_packages: list[BinaryPackage] | None = field(default_factory=list)

    def cmd(self, cmd: list[str] | str, **kwargs):
        """
        execute a command, auto-converts command strings/lists.
        use this to supports build dry-runs.
        """
        cmd_args : list[str] | str = cmd
        is_shell = kwargs.get("shell")
        if not is_shell and isinstance(cmd, str):
            cmd_args = shlex.split(cmd)
        elif is_shell and not isinstance(cmd, str):
            cmd_args = shlex.join(cmd)

        run_cmd(cmd_args, dry_run=self.dry_run, **kwargs)

    @property
    def install_dirs(self) -> dict[str, Path]:
        """ return { binary_package_name: install_directory } """
        return {
            pkg.name: self.install_base_dir / pkg.name
            for pkg in self.binary_packages
        }

    def select_packages(self, names: set[str]):
        """ only build those packages """
        if not names:
            self._selected_packages = None

        self._selected_packages = list()
        for pkg in self.binary_packages:
            if pkg.name in names:
                self._selected_packages.append(pkg)

    def filter_packages(self, package_filter: PackageFilter) -> None:
        """ apply filter to only build those packages """
        self.select_packages({pkg.name for pkg in
                              package_filter.get_packages(self.binary_packages)})

    def is_stage_completed(self, stage: BuildStage) -> bool:
        return stage in self._completed_stages

    def _mark_stage_done(self, stage: BuildStage) -> None:
        self._completed_stages.add(stage)
        # TODO: persist state in build dir

    def run(
        self,
        target_stage: BuildStage | None = None,
    ) -> None:
        for stage in BuildStage:
            print(f"debmagic: stage {stage!s}", end="")

            # skip done stages
            if self.is_stage_completed(stage):
                print(" already completed, skipping.")
                continue
            print(":")

            # run stage function from debian/rules.py
            if rules_stage_function := self.source_package.stage_functions.get(stage):
                print(f"debmagic:  running stage from rules file...")
                rules_stage_function(self)
                self._mark_stage_done(stage)

            else:
                # run stage function from first providing preset
                for preset in self.presets:
                    print(f"debmagic:  trying preset {preset}...")
                    if preset_stage_function := preset.get_stage(stage):
                        print("debmagic:  preset has function")
                        preset_stage_function(self)
                        self._mark_stage_done(stage)
                        break  # stop preset processing

            if not self.is_stage_completed(stage):
                breakpoint()
                raise RuntimeError(f"{stage!s} stage was never executed")

            if stage == target_stage:
                print(f"debmagic: target stage {stage!s} reached")
                break


class BuildError(RuntimeError):
    pass
