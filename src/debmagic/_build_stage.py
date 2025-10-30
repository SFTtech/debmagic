from enum import StrEnum


class BuildStage(StrEnum):
    prepare = "prepare"
    configure = "configure"
    build = "build"
    test = "test"
    install = "install"
    package = "package"
