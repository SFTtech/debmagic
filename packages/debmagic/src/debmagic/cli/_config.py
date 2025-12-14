import tomllib
from pathlib import Path
from typing import Sequence

from pydantic import BaseModel


class DebmagicConfig(BaseModel):
    # driver: BuildDriverType | None = None
    temp_build_dir = Path("/tmp/debmagic")


def merge_configs(configs: Sequence[DebmagicConfig]) -> DebmagicConfig:
    return configs[-1]


def load_config(config_path: Path) -> DebmagicConfig:
    with config_path.open("rb") as f:
        content = tomllib.load(f)
    return DebmagicConfig.model_validate(content)
