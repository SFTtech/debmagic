import re
import shutil
from pathlib import Path

from debmagic.common.utils import copy_file_if_exists

from .._config import DebmagicConfig
from .common import BuildConfig, BuildDriver, BuildDriverType, BuildMetadata, PackageDescription
from .config import BuildDriverConfig
from .driver_bare import BuildDriverBare
from .driver_docker import BuildDriverDocker
from .driver_lxd import BuildDriverLxd


def _create_driver(build_driver: BuildDriverType, build_config: BuildConfig, config: BuildDriverConfig) -> BuildDriver:
    match build_driver:
        case "docker":
            return BuildDriverDocker.create(config=build_config, driver_config=config.docker)
        case "lxd":
            return BuildDriverLxd.create(config=build_config, driver_config=config.lxd)
        case "bare":
            return BuildDriverBare.create(config=build_config, driver_config=config.bare)


def _driver_from_build_root(build_root: Path):
    build_metadata_path = build_root / "build.json"
    if not build_metadata_path.is_file():
        raise RuntimeError(f"{build_metadata_path} does not exist")
    try:
        metadata = BuildMetadata.model_validate_json(build_metadata_path.read_text())
    except:
        raise RuntimeError(f"{build_metadata_path} is invalid")

    match metadata.driver:
        case "docker":
            return BuildDriverDocker.from_build_metadata(metadata)
        case "lxd":
            return BuildDriverLxd.from_build_metadata(metadata)
        case "none":
            return BuildDriverBare.from_build_metadata(metadata)
        case _:
            raise RuntimeError(f"Unknown build driver {metadata.driver}")


def _write_build_metadata(config: BuildConfig, driver: BuildDriver):
    driver_metadata = driver.get_build_metadata()
    build_metadata_path = config.build_root_dir / "build.json"
    metadata = BuildMetadata(
        build_root=config.build_root_dir,
        source_dir=config.build_source_dir,
        driver=driver.driver_type(),
        driver_metadata=driver_metadata,
    )
    build_metadata_path.write_text(metadata.model_dump_json())


def _ignore_patterns_from_gitignore(gitignore_path: Path):
    if not gitignore_path.is_file():
        return None

    contents = gitignore_path.read_text().strip().splitlines()
    relevant_lines = filter(lambda line: not re.match(r"\s*#.*", line) and line.strip(), contents)
    return shutil.ignore_patterns(*relevant_lines)


def _get_package_build_root_and_identifier(config: DebmagicConfig, package: PackageDescription) -> tuple[str, Path]:
    package_identifier = f"{package.name}-{package.version}"
    build_root = config.temp_build_dir / package_identifier
    return package_identifier, build_root


def _prepare_build_env(config: DebmagicConfig, package: PackageDescription, output_dir: Path) -> BuildConfig:
    package_identifier, build_root = _get_package_build_root_and_identifier(config, package)
    if build_root.exists():
        shutil.rmtree(build_root)

    build_config = BuildConfig(
        package_identifier=package_identifier,
        source_dir=package.source_dir,
        output_dir=output_dir,
        build_root_dir=build_root,
        distro="debian",
        distro_version="trixie",
        dry_run=config.dry_run,
        sign_package=False,
    )

    # prepare build environment, create the build directory structure, copy the sources
    build_config.create_dirs()
    source_ignore_pattern = _ignore_patterns_from_gitignore(package.source_dir / ".gitignore")
    shutil.copytree(
        build_config.source_dir, build_config.build_source_dir, dirs_exist_ok=True, ignore=source_ignore_pattern
    )

    return build_config


def get_shell_in_build(config: DebmagicConfig, package: PackageDescription):
    _, build_root = _get_package_build_root_and_identifier(config, package)
    driver = _driver_from_build_root(build_root=build_root)
    driver.drop_into_shell()


def build(
    config: DebmagicConfig,
    package: PackageDescription,
    build_driver: BuildDriverType,
    output_dir: Path,
):
    build_config = _prepare_build_env(config=config, package=package, output_dir=output_dir)

    driver = _create_driver(build_driver, build_config, config.driver_config)
    _write_build_metadata(build_config, driver)
    try:
        driver.run_command(["apt-get", "-y", "build-dep", "."], cwd=build_config.build_source_dir, requires_root=True)
        driver.run_command(["dpkg-buildpackage", "-us", "-uc", "-ui", "-nc", "-b"], cwd=build_config.build_source_dir)
        if build_config.sign_package:
            pass
            # SIGN .changes and .dsc files
            # changes = *.changes / *.dsc
            # driver.run_command(["debsign", opts, changes], cwd=config.source_dir)
            # driver.run_command(["debrsign", opts, username, changes], cwd=config.source_dir)

        # TODO: copy packages to output directory
        copy_file_if_exists(source=build_config.build_source_dir / "..", glob="*.deb", dest=build_config.output_dir)
        copy_file_if_exists(
            source=build_config.build_source_dir / "..", glob="*.buildinfo", dest=build_config.output_dir
        )
        copy_file_if_exists(source=build_config.build_source_dir / "..", glob="*.changes", dest=build_config.output_dir)
        copy_file_if_exists(source=build_config.build_source_dir / "..", glob="*.dsc", dest=build_config.output_dir)
    except Exception as e:
        print(e)
        print(
            "Something failed during building -"
            " dropping into interactive shell in build environment for easier debugging"
        )
        driver.drop_into_shell()
        raise e
    finally:
        driver.cleanup()
