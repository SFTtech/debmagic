from .._build import Build

import subprocess


def configure(build: Build, args: list[str] | None = None):
    args = args or []

    subprocess.check_call(["./configure"] + args, cwd=build.source_dir)


def compile(build: Build, args: list[str] | None = None):
    args = args or []

    subprocess.check_call(["make"] + args, cwd=build.source_dir)


def install(build: Build, args: list[str] | None = None):
    args = args or []

    subprocess.check_call(
        ["make", f"DESTDIR={build.install_dir}", "install"] + args, cwd=build.source_dir
    )
