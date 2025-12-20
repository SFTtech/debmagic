import argparse
from pathlib import Path
from typing import ClassVar

from pydantic import Field
from pydantic_settings import (
    BaseSettings,
    CliApp,
    CliSettingsSource,
    PydanticBaseSettingsSource,
    TomlConfigSettingsSource,
)

from ._build_driver.config import BuildDriverConfig


# TODO: figure out how to get cli_avoid_json with the nested driver struct to work properly
class DebmagicConfig(BaseSettings, cli_kebab_case=True, cli_avoid_json=True):
    _config_file_paths: ClassVar[list[Path]] = []

    driver_config: BuildDriverConfig = BuildDriverConfig()

    temp_build_dir: Path = Field(
        default=Path("/tmp/debmagic"),
        description="Temporary directory on the local machine used as root directory for all package builds",
    )

    dry_run: bool = Field(
        default=False, description="don't actually run anything that changes the system/package state"
    )

    @classmethod
    def settings_customise_sources(
        cls,
        settings_cls: type[BaseSettings],
        init_settings: PydanticBaseSettingsSource,
        env_settings: PydanticBaseSettingsSource,
        dotenv_settings: PydanticBaseSettingsSource,
        file_secret_settings: PydanticBaseSettingsSource,
    ) -> tuple[PydanticBaseSettingsSource, ...]:
        # FIXME: once pydantic properly supports dynamic runtime paths for these kinds of use cases
        # use a more sane method of injecting additional paths to load
        # see https://github.com/pydantic/pydantic-settings/issues/259
        return tuple(TomlConfigSettingsSource(settings_cls, p) for p in cls._config_file_paths)


def get_config_argparser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(add_help=False)
    CliSettingsSource(DebmagicConfig, root_parser=parser)
    return parser


def load_config(args: argparse.Namespace, config_paths: list[Path]) -> DebmagicConfig:
    DebmagicConfig._config_file_paths = config_paths
    # parser = argparse.ArgumentParser(add_help=False)
    source = CliSettingsSource(DebmagicConfig)
    config = CliApp.run(DebmagicConfig, cli_args=args, cli_settings_source=source)

    return config
