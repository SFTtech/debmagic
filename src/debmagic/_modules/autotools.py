from .._build import Build
from .._utils import run_cmd


def configure(build: Build, args: list[str] | None = None):
    args = args or []

    configure_ac_path = build.source_dir / "configure.ac"
    if configure_ac_path.is_file():
        run_cmd(
            ["autoreconf", "--force", "--install", "--verbose"], cwd=build.source_dir
        )

    default_args = [
        "./configure",
        # f"--build={build.dpkg_architecture}", // TODO: get DEB_BUILD_GNU_TYPE
        "--prefix=/usr",
        "--includedir=${prefix}/include",
        "--mandir=${prefix}/share/man",
        "--infodir=${prefix}/share/info",
        "--sysconfdir=/etc",
        "--localstatedir=/var",
        "--disable-option-checking",
        "--disable-maintainer-mode",
        "--disable-dependency-tracking",
    ]

    # default_args += ["libexecdir=${prefix}/lib/" + sourcepackage()]

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
