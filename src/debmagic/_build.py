from dataclasses import dataclass
from pathlib import Path
from typing import Callable

from ._types import PresetT
from ._environment import BaseFrame


@dataclass
class Build:
    source_dir: Path
    install_dir: Path


type BuildStep = Callable[[Build], None]


def _get_func_from_preset(name: str, preset: PresetT) -> BuildStep | None:
    if preset is None:
        return None
    # TODO: allow sets, ...
    if isinstance(preset, list):
        for p in reversed(preset):
            func = getattr(p, name, None)
            if func is not None:
                return func
        return None

    return getattr(preset, name, None)


def build_package(
    build: Build,
    base_frame: BaseFrame,
    preset: PresetT,
) -> None:
    for step_name in ["configure", "compile", "install"]:
        if step := base_frame.local_vars.get(step_name):
            step(build)
        elif step := _get_func_from_preset(step_name, preset):
            step(build)
        else:
            # step not used
            pass
