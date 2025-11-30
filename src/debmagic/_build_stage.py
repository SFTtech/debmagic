from enum import StrEnum


class BuildStage(StrEnum):
    clean = "clean"
    prepare = "prepare"
    configure = "configure"
    build = "build"
    test = "test"
    install = "install"
    package = "package"
