from __future__ import annotations

import typing
from types import MethodType, ModuleType
from typing import TYPE_CHECKING, Callable, TypeVar, cast, overload

from ._build_stage import BuildStage

if TYPE_CHECKING:
    from ._build import Build
    from ._build_step import BuildStep
    from ._package import Package


class Preset:
    def __init__(self): ...

    def get_stage(self, stage: BuildStage) -> BuildStep | None:
        func: BuildStep = _get_member(self, stage)
        base_func: Callable[[Preset, Build], None] = _get_member(Preset, stage)

        # check if preset implementation overrides method
        if cast(MethodType, func).__func__ is base_func:
            return None

        return func

    def initialize(self, src_pkg: Package) -> None:
        """
        usually called when a Preset is set as preset to a SourcePackage.
        if you don't register pass the Preset to the package() function,
        you have to call this function yourself.
        """
        pass

    def clean(self, build: Build):
        """when dpkg wants to clean the source tree"""
        raise NotImplementedError()

    # now all build stages:

    def prepare(self, build: Build) -> None:
        raise NotImplementedError()

    def configure(self, build: Build) -> None:
        raise NotImplementedError()

    def build(self, build: Build) -> None:
        raise NotImplementedError()

    def test(self, build: Build) -> None:
        raise NotImplementedError()

    def install(self, build: Build) -> None:
        raise NotImplementedError()

    def package(self, build: Build) -> None:
        raise NotImplementedError()


type _PresetBuildStepClassMethod = Callable[[Preset, Build], None]


T = TypeVar("T", bound="Preset")


@overload
def _get_member(obj: T, stage: BuildStage) -> BuildStep:
    """Signature for when called with an *instance*."""
    ...


@overload
def _get_member(obj: type[T], stage: BuildStage) -> _PresetBuildStepClassMethod:
    """Signature for when called with the *class*."""
    ...


def _get_member(obj: T | type[T], stage: BuildStage) -> BuildStep | _PresetBuildStepClassMethod:
    match stage:
        case BuildStage.clean:
            return obj.clean
        case BuildStage.prepare:
            return obj.prepare
        case BuildStage.configure:
            return obj.configure
        case BuildStage.build:
            return obj.build
        case BuildStage.test:
            return obj.test
        case BuildStage.install:
            return obj.install
        case BuildStage.package:
            return obj.package


type PresetT = Preset | ModuleType
type PresetsT = PresetT | list[PresetT] | None


def as_presets(preset_elements: PresetsT) -> list[Preset]:
    presets: list[Preset] = []

    if isinstance(preset_elements, list):
        for preset_module in typing.cast(list[PresetT], preset_elements):
            presets.append(_as_preset(preset_module))

    elif preset_elements is not None:
        presets.append(_as_preset(preset_elements))

    return presets


def _as_preset(preset_entry: PresetT) -> Preset:
    if isinstance(preset_entry, ModuleType):
        return preset_entry.Preset()
    elif isinstance(preset_entry, Preset):
        return preset_entry  # user has provided preset instance
    elif preset_entry is Preset:
        return preset_entry()  # create new preset instance
    else:
        raise ValueError(f"invalid preset element {preset_entry!r}")
