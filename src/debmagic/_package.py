import argparse
import shutil

from debian import deb822


from ._build import Build, build_package
from ._types import PresetT
from ._environment import get_base_frame


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


def package(preset: PresetT = None):
    args = _parse_args()

    # get our function caller's file directory
    base_frame = get_base_frame()

    # TODO move into Build
    packages = list()
    for block in deb822.DebControl.iter_paragraphs(
        (base_frame.path / "debian/control").open()
    ):
        if "Package" in block:
            packages.append(block)

    assert len(packages) == 1  # TODO support >1
    install_dir = base_frame.path / "debian" / packages[0]["Package"]

    if install_dir.is_dir():
        shutil.rmtree(install_dir)

    install_dir.mkdir(exist_ok=True)

    build = Build(
        source_dir=base_frame.path,
        install_dir=install_dir,
    )

    match args.operation:
        case "clean":  # undo whatever "build" and "binary" did
            raise NotImplementedError()
        case "build":  # configure and compile
            raise NotImplementedError()
        case "build-arch":
            # package from d/control with Architecture != all
            raise NotImplementedError()
        case "build-indep":
            # package from d/control with Architecture == all
            raise NotImplementedError()
        case "binary":  # create binary package(s) from source package
            build_package(build, base_frame, preset)
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
