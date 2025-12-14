from typing import Callable

from ._build import Build

type BuildStep = Callable[[Build], None]
