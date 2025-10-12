import shutil
from dataclasses import dataclass
from pathlib import Path
from typing import Callable

from debmagic._utils import run_cmd

from ._rules_file import RulesFile
from ._types import PresetT
from ._utils import Namespace


@dataclass
class Build:
    source_dir: Path
    install_dir: Path
    architecture_target: str
    architecture_host: str
    flags: Namespace
    parallel: int
    prefix: str = "/usr"

type BuildStep = Callable[[Build], None]


class BuildError(RuntimeError):
    pass


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


def strip(build: Build):
    run_cmd(["dh_dwz", "-a"], cwd=build.source_dir)
    run_cmd(["dh_strip", "-a"], cwd=build.source_dir)


def gen_shlibs(build: Build):
    run_cmd(["dh_makeshlibs", "-a"], cwd=build.source_dir)
    run_cmd(["dh_shlibdeps", "-a"], cwd=build.source_dir)


def install_deb(build: Build):
    run_cmd(["dh_installdeb"], cwd=build.source_dir)


def gen_control_file(build: Build):
    run_cmd(["dh_gencontrol"], cwd=build.source_dir)


def make_md5sums(build: Build):
    run_cmd(["dh_md5sums"], cwd=build.source_dir)


def build_deb(build: Build):
    run_cmd(["dh_builddeb"], cwd=build.source_dir)


def clean_package(
    build: Build,
    rules_frame: RulesFile,
    preset: PresetT,
) -> None:

    # clean install dir
    if build.install_dir.is_dir():
        shutil.rmtree(build.install_dir)


def build_package(
    build: Build,
    rules_frame: RulesFile,
    preset: PresetT,
) -> None:
    build.install_dir.mkdir(exist_ok=True)

    for step_name in ["configure", "compile", "install"]:
        if step := rules_frame.local_vars.get(step_name):
            step(build)
        elif step := _get_func_from_preset(step_name, preset):
            step(build)
        else:
            # step not used
            pass

    strip(build)
    gen_shlibs(build)
    install_deb(build)
    gen_control_file(build)
    make_md5sums(build)
    build_deb(build)
