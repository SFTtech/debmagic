from .._build import Build, BuildError
from .._utils import run_cmd


def autoreconf(build: Build):
    configure_ac_path = build.source_dir / "configure.ac"
    if configure_ac_path.is_file():
        run_cmd(
            ["autoreconf", "--force", "--install", "--verbose"], cwd=build.source_dir
        )


def configure(build: Build, args: list[str] | None = None):
    args = args or []

    if not (build.source_dir / "configure").is_file():
        raise BuildError("no 'configure' file in build root - perhaps run autotools.autoreconf()?")

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
        "--disable-option-checking",  # TODO: if not strict-mode
        "--disable-maintainer-mode",
        "--disable-dependency-tracking",
        #"--with-bugurl=https://bugs.launchpad.net/ubuntu/+source/${srcpkg}",
    ]

    if multiarch := build.flags["DEB_HOST_MULTIARCH"]:
        default_args.append(f"--libdir=${{prefix}}/lib/{multiarch}")
        default_args.append(f"--libexecdir=${{prefix}}/lib/{multiarch}")
    else:
        default_args.append("--libexecdir=${prefix}/lib")

    # cross-building
    default_args.append(f"--build={build.architecture_target}")
    if build.architecture_target != build.architecture_host:
        default_args.append(f"--host={build.architecture_target}")

    run_cmd(
        default_args + args,
        cwd=build.source_dir,
    )


def compile(build: Build, args: list[str] | None = None):
    args = args or []

    run_cmd(["make"] + args, cwd=build.source_dir)


def install(build: Build, args: list[str] | None = None):
    args = args or []

    run_cmd(
        ["make", f"DESTDIR={build.install_dir}", "install"] + args, cwd=build.source_dir
    )

# TODO: def test(): run make test or make check
