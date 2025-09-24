from .._build import Build

from . import autotools


def configure(build: Build, args: list[str] | None = None):
    autotools.configure(build, args)


def compile(build: Build, args: list[str] | None = None):
    autotools.compile(build, args)


def install(build: Build, args: list[str] | None = None):
    autotools.install(build, args)
