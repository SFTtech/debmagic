from __future__ import annotations

import argparse
import inspect
import multiprocessing
import os
import typing
from dataclasses import dataclass, field
from enum import Flag, auto
from pathlib import Path
from types import FunctionType
from typing import Callable, ParamSpec, TypeVar

from debmagic.common.package import BinaryPackage, SourcePackage
from debmagic.common.package_version import PackageVersion
from debmagic.common.utils import Namespace, disable_output_buffer

from ._build import Build
from ._build_order import BuildOrder
from ._build_stage import BuildStage
from ._build_step import BuildStep
from ._dpkg import buildflags
from ._preset import Preset, PresetsT, as_presets
from ._rules_file import RulesFile, find_rules_file
from ._types import CustomFuncArg, CustomFuncArgsT


@dataclass
class CustomFunction:
    fun: Callable
    args: CustomFuncArgsT


def _parse_args(custom_functions: dict[str, CustomFunction] = {}):
    cli = argparse.ArgumentParser()
    sp = cli.add_subparsers(dest="operation")

    sp.add_parser("help")

    common_cli = argparse.ArgumentParser(add_help=False)
    common_cli.add_argument(
        "--dry-run",
        action="store_true",
        help="don't actually run anything that changes the system/package state",
    )

    # debian-required "targets":
    sp.add_parser("clean", parents=[common_cli])
    sp.add_parser("patch", parents=[common_cli])
    sp.add_parser("build", parents=[common_cli])
    sp.add_parser("build-arch", parents=[common_cli])
    sp.add_parser("build-indep", parents=[common_cli])
    sp.add_parser("binary", parents=[common_cli])
    sp.add_parser("binary-arch", parents=[common_cli])
    sp.add_parser("binary-indep", parents=[common_cli])

    # goal: have fine-grain control to trigger (and resume!) those gentoo has:
    # pkg_pretend
    # pkg_nofetch
    # pkg_setup
    # src_unpack
    # src_prepare
    # src_configure
    # src_compile
    # src_test
    # src_install
    # pkg_pack?
    # pkg_preinst  # could be in py
    # pkg_postinst
    # pkg_prerm
    # pkg_postrm
    # pkg_config   # enhanced debconf

    # register custom function names
    for name, func in custom_functions.items():
        func_parser = sp.add_parser(name.replace("_", "-"))
        for arg in func.args.values():
            if arg.default is not None:
                arg_name = f"--{arg.name.replace('_', '-')}"
            else:
                arg_name = arg.name
            func_parser.add_argument(
                arg_name,
                type=arg.type,
                default=arg.default,
            )

    return cli, cli.parse_args()


class PackageFilter(Flag):
    architecture_specific = auto()
    architecture_independent = auto()

    def get_packages(self, binary_packages: list[BinaryPackage]) -> list[BinaryPackage]:
        ret: list[BinaryPackage] = []

        for pkg in binary_packages:
            if self.architecture_independent and pkg.arch_dependent:
                ret.append(pkg)
            if self.architecture_specific and not pkg.arch_dependent:
                ret.append(pkg)

        return ret


P = ParamSpec("P")
R = TypeVar("R")


@dataclass
class SourcePackageBuild:
    source_package: SourcePackage
    rules_file: RulesFile
    presets: list[Preset]
    buildflags: Namespace
    version: PackageVersion
    stage_functions: dict[BuildStage, BuildStep] = field(default_factory=dict)
    custom_functions: dict[str, CustomFunction] = field(default_factory=dict)

    def __post_init__(self):
        for preset in self.presets:
            preset.initialize(self)

    @property
    def default(self) -> Namespace:
        raise NotImplementedError("access to preset default functions")

    @property
    def base_dir(self) -> Path:
        return self.rules_file.package_dir

    def stage(self, func: BuildStep) -> BuildStep:
        """
        decorator to register a packaging stage function
        """
        name = typing.cast(FunctionType, func).__code__.co_name
        stage = BuildStage(name)
        self.stage_functions[stage] = func
        return func

    def custom_function(self, func: Callable[P, R]) -> Callable[P, R]:
        """
        decorator to register a function to be callable in pack().
        arguments and their defaults get added to the argparsing.

        usage in debian/rules.py:

        pkg = package(...)
        @pkg.custom_function
        def something(arg: str = 'stuff'):
            ...
        """
        name = typing.cast(FunctionType, func).__code__.co_name

        # find arguments and its types to guess argparsing
        args_raw = inspect.getfullargspec(func)
        args: CustomFuncArgsT = {}
        default_count = 0 if not args_raw.defaults else len(args_raw.defaults)
        default_start = len(args_raw.args) - default_count
        for idx, arg in enumerate(args_raw.args):
            default = None
            if args_raw.defaults and idx >= default_start:
                default = args_raw.defaults[idx - default_start]
            arg_type = args_raw.annotations[arg]
            if isinstance(arg_type, str):
                arg_type = eval(arg_type)  # when __future__.annotations return strings
            args[arg] = CustomFuncArg(arg, arg_type, default)

        self.custom_functions[name] = CustomFunction(func, args)
        return func

    def pack(self):
        cli, args = _parse_args(self.custom_functions)

        build = Build(
            presets=self.presets,
            source_package=self,
            source_dir=self.base_dir,
            binary_packages=self.source_package.binary_packages,
            install_base_dir=self.rules_file.package_dir / "debian",  # + added binary package
            architecture_target=self.buildflags.DEB_BUILD_GNU_TYPE,
            architecture_host=self.buildflags.DEB_HOST_GNU_TYPE,
            flags=self.buildflags,
            parallel=multiprocessing.cpu_count(),  # TODO
            prefix=Path("/usr"),  # TODO
            dry_run=args.dry_run,
        )

        match args.operation:
            case "help":
                cli.print_help()
                cli.exit(0)
            case "clean":
                # undo whatever "build" and "binary" did
                build.run(BuildStage.clean)
            case "build":
                # configure and compile
                build.run(BuildStage.build)
            case "build-arch":
                # package from d/control with Architecture != all
                build.filter_packages(PackageFilter.architecture_specific)
                build.run(BuildStage.build)
            case "build-indep":
                # package from d/control with Architecture == all
                build.filter_packages(PackageFilter.architecture_independent)
                build.run(BuildStage.build)
            case "binary":
                # create binary package(s) from source package
                build.run(BuildStage.package)
            case "binary-arch":
                # architecture dependent binary package(s)
                build.filter_packages(PackageFilter.architecture_specific)
                build.run(BuildStage.package)
            case "binary-indep":
                # non-architecture specific binary package(s)
                build.filter_packages(PackageFilter.architecture_independent)
                build.run(BuildStage.package)
            case None:
                cli.print_help()
                cli.exit()
            case _:
                # custom functions
                if func := self.custom_functions.get(args.operation.replace("-", "_")):
                    # pass all requested parameters
                    func_args = {k: vars(args)[k] for k in func.args.keys()}
                    func.fun(**func_args)
                else:
                    print(f"call to unknown operation {args.operation!r}\n")
                    cli.print_help()
                    cli.exit(1)


def package(
    preset: PresetsT = None,
    maint_options: str | None = None,
    build_order: BuildOrder = BuildOrder.stages,
) -> SourcePackageBuild:
    """
    provides the packaging environment.

    in the future, could also generate control file contents via its arguments
    instead of reading it.
    """

    disable_output_buffer()

    # get our function caller's file directory
    rules_file = find_rules_file()

    # which build presets to apply
    presets: list[Preset] = as_presets(preset)

    # apply default preset last
    from ._module.default import Preset as DefaultPreset

    presets.append(DefaultPreset())

    # set buildflags as environment variables
    flags, version = buildflags.get_pkg_env(rules_file.package_dir, maint_options=maint_options)
    os.environ.update(flags)

    source_package = SourcePackage.from_control_file(rules_file.package_dir / "debian/control")
    build = SourcePackageBuild(
        source_package=source_package,
        rules_file=rules_file,
        presets=presets,
        buildflags=Namespace(**flags),
        version=version,
    )
    return build
