from enum import Flag, auto

from debmagic.common.package import BinaryPackage


class PackageFilter(Flag):
    architecture_specific = auto()
    architecture_independent = auto()

    def get_packages(self, binary_packages: list[BinaryPackage]) -> list[BinaryPackage]:
        ret: list[BinaryPackage] = []

        for pkg in binary_packages:
            if self.architecture_independent and pkg.arch_dependent:
                ret.append(pkg)
            if self.architecture_specific and not pkg.arch_dependent:
                ret.append(pkg)

        return ret
