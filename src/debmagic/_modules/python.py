from .._build import Build
from .._utils import run_cmd


def configure(build: Build, args: list[str] | None = None):
    args = args or []
    run_cmd(["pybuild", "--configure"] + args, cwd=build.source_dir)


def compile(build: Build, args: list[str] | None = None):
    args = args or []
    run_cmd(["pybuild", "--build"] + args, cwd=build.source_dir)


def install(build: Build, args: list[str] | None = None):
    args = args or []
    run_cmd(["pybuild", "--install"] + args, cwd=build.source_dir)
    run_cmd(["dh_python3"])
