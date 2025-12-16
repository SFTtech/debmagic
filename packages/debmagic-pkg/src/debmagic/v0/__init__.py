from debmagic.common.utils import run_cmd

from ._build import Build
from ._module import autotools, dh
from ._package import package
from ._preset import Preset

__all__ = [
    "Build",
    "Preset",
    "autotools",
    "dh",
    "package",
    "run_cmd",
]
