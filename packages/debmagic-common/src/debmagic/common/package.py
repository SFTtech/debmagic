from dataclasses import dataclass
from pathlib import Path
from typing import Self

from debian import deb822


@dataclass
class BinaryPackage:
    name: str
    ctrl: deb822.Packages
    arch_dependent: bool


@dataclass
class SourcePackage:
    name: str
    binary_packages: list[BinaryPackage]

    @classmethod
    def from_control_file(cls, control_file_path: Path) -> Self:
        src_pkg: Self | None = None
        # which binary packages should be produced?
        bin_pkgs: list[BinaryPackage] = []

        for block in deb822.DebControl.iter_paragraphs(
            control_file_path.open(),
            use_apt_pkg=False,  # don't depend on python3-apt for now.
        ):
            if "Source" in block:
                if src_pkg is not None:
                    raise RuntimeError("encountered multiple Source: blocks in control file")
                src_name = block["Source"]
                src_pkg = cls(
                    src_name,
                    bin_pkgs,
                )

            if "Package" in block:
                bin_pkg = BinaryPackage(
                    name=block["Package"],
                    ctrl=block,
                    arch_dependent=block["Architecture"] != "all",
                )
                bin_pkgs.append(bin_pkg)

        if src_pkg is None:
            raise RuntimeError("no 'Source:' package defined in control file")

        if not bin_pkgs:
            raise RuntimeError("no binary 'Package:' defined in control file")

        return src_pkg
