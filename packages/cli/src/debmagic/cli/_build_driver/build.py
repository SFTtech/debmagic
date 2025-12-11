import re
import shutil
from pathlib import Path

from debmagic.common.utils import copy_file_if_exists

from .common import BuildConfig, BuildDriver, BuildDriverType, BuildMetadata, PackageDescription
from .driver_docker import BuildDriverDocker
from .driver_lxd import BuildDriverLxd
from .driver_none import BuildDriverNone

DEBMAGIC_TEMP_BUILD_PARENT_DIR = Path("/tmp/debmagic")


def _create_driver(build_driver: BuildDriverType, config: BuildConfig) -> BuildDriver:
    match build_driver:
        case "docker":
            return BuildDriverDocker.create(config=config)
        case "lxd":
            return BuildDriverLxd.create(config=config)
        case "none":
            return BuildDriverNone.create(config=config)


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
            return BuildDriverNone.from_build_metadata(metadata)
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


def _get_package_build_root_and_identifier(package: PackageDescription) -> tuple[str, Path]:
    package_identifier = f"{package.name}-{package.version}"
    build_root = DEBMAGIC_TEMP_BUILD_PARENT_DIR / package_identifier
    return package_identifier, build_root


def _prepare_build_env(package: PackageDescription, output_dir: Path, dry_run: bool) -> BuildConfig:
    package_identifier, build_root = _get_package_build_root_and_identifier(package)
    if build_root.exists():
        shutil.rmtree(build_root)

    config = BuildConfig(
        package_identifier=package_identifier,
        source_dir=package.source_dir,
        output_dir=output_dir,
        build_root_dir=build_root,
        distro="debian",
        distro_version="trixie",
        dry_run=dry_run,
        sign_package=False,
    )

    # prepare build environment, create the build directory structure, copy the sources
    config.create_dirs()
    source_ignore_pattern = _ignore_patterns_from_gitignore(package.source_dir / ".gitignore")
    shutil.copytree(config.source_dir, config.build_source_dir, dirs_exist_ok=True, ignore=source_ignore_pattern)

    return config


def get_shell_in_build(package: PackageDescription):
    _, build_root = _get_package_build_root_and_identifier(package)
    driver = _driver_from_build_root(build_root=build_root)
    driver.drop_into_shell()


def build(
    package: PackageDescription,
    build_driver: BuildDriverType,
    output_dir: Path,
    dry_run: bool = False,
):
    config = _prepare_build_env(package=package, output_dir=output_dir, dry_run=dry_run)

    driver = _create_driver(build_driver, config)
    _write_build_metadata(config, driver)
    try:
        driver.run_command(["apt-get", "-y", "build-dep", "."], cwd=config.build_source_dir, requires_root=True)
        driver.run_command(["dpkg-buildpackage", "-us", "-uc", "-ui", "-nc", "-b"], cwd=config.build_source_dir)
        if config.sign_package:
            pass
            # SIGN .changes and .dsc files
            # changes = *.changes / *.dsc
            # driver.run_command(["debsign", opts, changes], cwd=config.source_dir)
            # driver.run_command(["debrsign", opts, username, changes], cwd=config.source_dir)

        # TODO: copy packages to output directory
        copy_file_if_exists(source=config.build_source_dir / "..", glob="*.deb", dest=config.output_dir)
        copy_file_if_exists(source=config.build_source_dir / "..", glob="*.buildinfo", dest=config.output_dir)
        copy_file_if_exists(source=config.build_source_dir / "..", glob="*.changes", dest=config.output_dir)
        copy_file_if_exists(source=config.build_source_dir / "..", glob="*.dsc", dest=config.output_dir)
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
