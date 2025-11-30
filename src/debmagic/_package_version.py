import re
from dataclasses import dataclass
from typing import Self


@dataclass
class PackageVersion:
    """
    debian version string, split into various subparts.
    """

    #: distro packaging override base version (default is 0)
    epoch: str

    #: upstream package version
    upstream: str

    #: packaging (linux distro) revision
    revision: str

    @property
    def version(self) -> str:
        ret = ""
        if self.epoch != "0":
            ret += f"{self.epoch}:"
        ret += self.upstream
        if self.revision:
            ret += f"-{self.revision}"
        return ret

    @property
    def epoch_upstream(self) -> str:
        """
        distro epoch plus upstream version
        """
        if self.epoch:
            return f"{self.epoch}:{self.upstream}"
        else:
            return self.upstream

    @property
    def upstream_revision(self) -> str:
        """
        upstream version including distro package revision
        """
        if self.revision:
            return f"{self.upstream}-{self.revision}"
        else:
            return self.upstream

    @classmethod
    def from_str(cls, version: str) -> Self:
        epoch_upstream = re.sub(r"^(.*?)(-[^-]*)?$", r"\1", version)
        return cls(
            # epoch = distro packaging override base version (default is 0)
            # pkg-info.mk uses the full version if no epoch is in it.
            # instead, we return "0" as oritinally intended if no epoch is in version.
            epoch="0" if ":" not in version else re.sub(r"^([0-9]+):.*$", r"\1", version),
            upstream=re.sub(r"^([0-9]*:)?(.*?)$", r"\2", epoch_upstream),
            revision=re.sub(r"^.*?(-([^-]*))?$", r"\2", version),
        )
