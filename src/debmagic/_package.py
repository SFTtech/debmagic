import argparse
import multiprocessing
import os
from pathlib import Path

from debian import deb822

from ._build import Build, build_package, clean_package
from ._dpkg import buildflags
from ._rules_file import RulesFile, find_rules_file
from ._types import PresetT
from ._utils import Namespace, disable_output_buffer


def _parse_args():
    cli = argparse.ArgumentParser()
    sp = cli.add_subparsers(dest="operation")

    # debian-required "targets":
    sp.add_parser("clean")
    sp.add_parser("patch")
    sp.add_parser("build")
    sp.add_parser("build-arch")
    sp.add_parser("build-indep")
    sp.add_parser("binary")
    sp.add_parser("binary-arch")
    sp.add_parser("binary-indep")

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
    # pkg_preinst
    # pkg_postinst
    # pkg_prerm
    # pkg_postrm
    # pkg_config

    return cli.parse_args()


class SourcePackage:
    def __init__(
        self,
        rules_file: RulesFile,
        preset: PresetT,
        binary_packages: list[deb822.Packages],
        buildflags: Namespace,
    ):
        self._rules_file: RulesFile = rules_file
        self._preset: PresetT = preset
        self._binary_packages: list[deb822.Packages] = binary_packages
        self._buildflags: Namespace = buildflags

    @property
    def buildflags(self) -> Namespace:
        return self._buildflags

    @property
    def default(self) -> Namespace:
        raise NotImplementedError("access to preset default functions")

    @property
    def base_dir(self) -> Path:
        return self._rules_file.package_dir

    def pack(self):
        args = _parse_args()

        # TODO move install dir preparation to build
        # TODO multi package support
        install_dir = (
            self._rules_file.package_dir / "debian" / self._binary_packages[0]["Package"]
        )

        build = Build(
            source_dir=self._rules_file.package_dir,
            install_dir=install_dir,
            architecture_target=self._buildflags.DEB_BUILD_GNU_TYPE,
            architecture_host=self._buildflags.DEB_HOST_GNU_TYPE,
            flags=self._buildflags,
            parallel=multiprocessing.cpu_count()
        )

        match args.operation:
            case "clean":  # undo whatever "build" and "binary" did
                clean_package(build, self._rules_file, self._preset)
            case "build":  # configure and compile
                raise NotImplementedError()
            case "build-arch":
                # package from d/control with Architecture != all
                raise NotImplementedError()
            case "build-indep":
                # package from d/control with Architecture == all
                raise NotImplementedError()
            case "binary":  # create binary package(s) from source package
                build_package(build, self._rules_file, self._preset)
                # TODO: trigger binary-arch and binary-indep
            case "binary-arch":  # architecture dependent binary package(s)
                # TODO need successful build-arch
                raise NotImplementedError()
            case "binary-indep":  # non-architecture specific binary package(s)
                # TODO need successful build-indep
                raise NotImplementedError()
            case None:
                raise NotImplementedError("default: show help?")
            case _:
                raise NotImplementedError()


def package(preset: PresetT = None, maint_options: str | None = None):
    """
    provides the packaging environment.

    in the future, could also generate control file contents via its arguments
    instead of reading it.
    """

    disable_output_buffer()

    # get our function caller's file directory
    rules_file = find_rules_file()

    # which binary packages should be produced?
    bin_pkgs = list()
    for block in deb822.DebControl.iter_paragraphs(
        (rules_file.package_dir / "debian/control").open()
    ):
        if "Package" in block:
            bin_pkgs.append(block)

    if len(bin_pkgs) != 1:  # TODO support >1
        raise NotImplementedError("Building more than one package is not supported yet")

    # set buildflags as environment variables
    flags = buildflags.get_flags(rules_file.package_dir, maint_options=maint_options)
    os.environ.update(flags)

    return SourcePackage(
        rules_file,
        preset,
        bin_pkgs,
        buildflags=Namespace(**flags),
    )
