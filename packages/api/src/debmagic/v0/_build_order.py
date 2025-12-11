from enum import StrEnum


class BuildOrder(StrEnum):
    stages = "stages"
    """ iterate over all build stages, then work for all selected binary packages """
    packages = "packages"
    """ iterate over selected binary packages and perform each stage for it """
