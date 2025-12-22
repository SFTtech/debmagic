"""
GNU Autotools module

for the "configure-make" build work flow.

preset tries to execute:
- make clean
- configure
- make
- make DESTDIR=... install

functions included:
- autoreconf(): for generating `configure` from `configure.ac`
- clean(): to call `make clean` (or another target)
- configure(): to call `./configure <args>`
- build(): calls `make -j<jobs>`
- test(): calls `make test`
- install(): calls `make DESTDIR=<dir> install`
"""

import shlex
from pathlib import Path
from typing import Iterable

from debmagic.common.utils import run_cmd

from .._build import Build, BuildError
from .._preset import Preset as PresetBase


class Preset(PresetBase):
    def clean(self, build: Build) -> None:
        if not _has_makefile(build.source_dir):
            return
        clean(build)

    def configure(self, build: Build, args: list[str] | None = None) -> None:
        if not _has_configure(build.source_dir):
            return
        configure(build, args or [])

    def build(self, build: Build, args: list[str] | None = None) -> None:
        if not _has_makefile(build.source_dir):
            return
        _build(build, args or [])

    def test(self, build: Build) -> None:
        if not _has_makefile(build.source_dir):
            return
        test(build)

    def install(self, build: Build):
        if not _has_makefile(build.source_dir):
            return
        install(build)


def autoreconf(build: Build) -> None:
    if not (build.source_dir / "configure.ac").is_file():
        raise BuildError("no 'configure.ac' file found in build root for `autoreconf`")

    build.cmd(["autoreconf", "--force", "--install", "--verbose"], cwd=build.source_dir)


def clean(build: Build, target: str | None = None) -> None:
    if not _has_makefile(build.source_dir):
        raise BuildError("no 'makefile' file found in build root for cleaning")

    if not target:
        target = _make_test_targets(("distclean", "realclean", "clean"), cwd=build.source_dir)

    if target:
        build.cmd(["make", target])


def configure(build: Build, args: list[str] | str | None = None):
    if not _has_configure(build.source_dir):
        raise BuildError("no 'configure' file in build root - perhaps run autotools.autoreconf()?")

    match args:
        case None:
            custom_args = []
        case str():
            custom_args = shlex.split(args)
        case list():
            custom_args = args

    # as autotools-dev/README.Debian recommends
    default_args = [
        "./configure",
        f"--prefix={build.prefix}",
        "--includedir=${prefix}/include",
        "--mandir=${prefix}/share/man",
        "--infodir=${prefix}/share/info",
        "--sysconfdir=/etc",
        "--localstatedir=/var",
        "--runstatedir=/run",
        # "--disable-option-checking",  # TODO: activate if not strict-mode?
        "--disable-maintainer-mode",
        "--disable-dependency-tracking",
        # "--with-bugurl=https://bugs.launchpad.net/ubuntu/+source/${srcpkg}",
    ]

    if multiarch := build.package.build_env["DEB_HOST_MULTIARCH"]:
        default_args.append(f"--libdir=${{prefix}}/lib/{multiarch}")
        default_args.append(f"--libexecdir=${{prefix}}/lib/{multiarch}")
    else:
        default_args.append("--libexecdir=${prefix}/lib")

    # cross-building
    default_args.append(f"--build={build.architecture_target}")
    if build.architecture_target != build.architecture_host:
        default_args.append(f"--host={build.architecture_target}")

    build.cmd(
        [*default_args, *custom_args],
        cwd=build.source_dir,
    )
    # TODO: show some config.log if configure failed


def build(build: Build, args: list[str] = []) -> None:
    if not _has_makefile(build.source_dir):
        raise BuildError("no 'makefile' file in build root - perhaps run autotools.configure()?")

    args = [f"-j{build.parallel}", *args]
    build.cmd(["make", *args], cwd=build.source_dir)


# otherwise the preset function argument name has to be adjusted
# which is an invalid method override then
_build = build


def test(build: Build, target: str | None = None) -> None:
    if not _has_makefile(build.source_dir):
        raise BuildError("no 'makefile' file in build root - perhaps run autotools.configure()?")

    if not target:
        target = _make_test_targets(("test", "check"), cwd=build.source_dir)

    if target:
        build.cmd(["make", target])


def install(build: Build, target: str = "install") -> None:
    # TODO: figure out installdir handling for multi package builds
    destdir = build.install_dirs[build.package.source_package.name]
    build.cmd(["make", f"DESTDIR={destdir}", target], cwd=build.source_dir)


def _has_makefile(path: Path) -> bool:
    return any((path / makefile).is_file() for makefile in ("GNUmakefile", "makefile", "Makefile"))


def _has_configure(path: Path) -> bool:
    return (path / "configure").is_file()


def _make_test_targets(candidates: Iterable[str], cwd: Path) -> str | None:
    """
    test for makefile target availabilty: https://www.gnu.org/software/make/manual/html_node/Running.html
    """
    for candidate in candidates:
        attempt = run_cmd(f"make -q {candidate}", cwd=cwd, capture_output=True)
        if attempt.returncode == 1:
            return candidate
    return None
