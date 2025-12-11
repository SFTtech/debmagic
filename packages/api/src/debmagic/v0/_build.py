from __future__ import annotations

import typing
from dataclasses import dataclass, field
from pathlib import Path
from typing import Sequence

from debmagic.common.utils import run_cmd

from ._build_stage import BuildStage

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

    def cmd(self, cmd: Sequence[str] | str, **kwargs):
        """
        execute a command, auto-converts command strings/lists.
        use this to supports build dry-runs.
        """
        run_cmd(cmd, dry_run=self.dry_run, **kwargs)

    @property
    def install_dirs(self) -> dict[str, Path]:
        """return { binary_package_name: install_directory }"""
        return {pkg.name: self.install_base_dir / pkg.name for pkg in self.binary_packages}

    def select_packages(self, names: set[str]):
        """only build those packages"""
        self.binary_packages = []

        for pkg in self.source_package.binary_packages:
            if pkg.name in names:
                self.binary_packages.append(pkg)

    def filter_packages(self, package_filter: PackageFilter) -> None:
        """apply filter to only build those packages"""
        self.select_packages({pkg.name for pkg in package_filter.get_packages(self.source_package.binary_packages)})

    def filtered_binary_packages(self, names: set[str]) -> typing.Iterator[BinaryPackage]:
        for pkg in self.binary_packages:
            if pkg.name in names:
                yield pkg

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
                print("debmagic:  running stage from rules file...")
                rules_stage_function(self)
                self._mark_stage_done(stage)

            else:
                # run stage function from first providing preset
                for preset in self.presets:
                    print(f"debmagic:  trying preset {preset}...")
                    if preset_stage_function := preset.get_stage(stage):
                        print("debmagic:   running stage from preset")
                        preset_stage_function(self)
                        self._mark_stage_done(stage)
                        break  # stop preset processing

            if not self.is_stage_completed(stage):
                raise RuntimeError(f"{stage!s} stage was never executed")

            if stage == target_stage:
                print(f"debmagic: target stage {stage!s} reached")
                break


class BuildError(RuntimeError):
    pass
