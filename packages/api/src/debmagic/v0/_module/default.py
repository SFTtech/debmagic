"""
fallback for all build stages.
"""

import shutil

from .._build import Build
from .._preset import Preset as BasePreset
from .dh import Preset as DHPreset


class Preset(BasePreset):
    """
    default preset, always active and tried last.
    just implements clean, install and package by using debhelper for it.
    if you don't want it, supply a preset that provides clean, install, package.

    TODO: some day should not depend on debhelper at all, instead it should provide
    debmagic's dead simple default packaging behavior.
    """

    def __init__(self):
        super().__init__()
        self._dh_preset = DHPreset()

    def clean(self, build: Build) -> None:
        # clean install dirs
        for install_dir in build.install_dirs.values():
            if install_dir.is_dir():
                shutil.rmtree(install_dir)

        # run dh's clean sequence
        self._dh_preset.clean(build)

    def prepare(self, build: Build) -> None:
        return

    def configure(self, build: Build) -> None:
        return

    def build(self, build: Build) -> None:
        return

    def test(self, build: Build) -> None:
        return

    def install(self, build: Build) -> None:
        # run dh's install sequence
        self._dh_preset.install(build)

    def package(self, build: Build) -> None:
        self._dh_preset.package(build)
