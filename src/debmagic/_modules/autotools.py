from .._build import Build
from .._utils import run_cmd


def configure(build: Build, args: list[str] | None = None):
    args = args or []

    configure_ac_path = build.source_dir / "configure.ac"
    if configure_ac_path.is_file():
        run_cmd(
            ["autoreconf", "--force", "--install", "--verbose"], cwd=build.source_dir
        )

    run_cmd(["./configure"] + args, cwd=build.source_dir)


def compile(build: Build, args: list[str] | None = None):
    args = args or []

    run_cmd(["make"] + args, cwd=build.source_dir)


def install(build: Build, args: list[str] | None = None):
    args = args or []

    run_cmd(
        ["make", f"DESTDIR={build.install_dir}", "install"] + args, cwd=build.source_dir
    )
