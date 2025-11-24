import re
import shutil
from pathlib import Path

from debmagic._build_driver.driver_docker import BuildDriverDocker
from debmagic._build_driver.driver_lxd import BuildDriverLxd
from debmagic._build_driver.driver_none import BuildDriverNone

from .common import BuildConfig, BuildDriver, BuildDriverType

DEBMAGIC_TEMP_BUILD_PARENT_DIR = Path("/tmp/debmagic")


def _create_driver(build_driver: BuildDriverType, config: BuildConfig) -> BuildDriver:
    match build_driver:
        case "docker":
            return BuildDriverDocker.create(config=config)
        case "lxd":
            return BuildDriverLxd.create(config=config)
        case "none":
            return BuildDriverNone.create(config=config)


def _ignore_patterns_from_gitignore(gitignore_path: Path):
    if not gitignore_path.is_file():
        return None

    contents = gitignore_path.read_text().strip().splitlines()
    relevant_lines = filter(lambda line: not re.match(r"\s*#.*", line) and line.strip(), contents)
    return shutil.ignore_patterns(*relevant_lines)


def _prepare_build_env(source_dir: Path, output_dir: Path, dry_run: bool) -> BuildConfig:
    package_name = "debmagic"  # TODO
    package_version = "0.1.0"  # TODO

    package_identifier = f"{package_name}-{package_version}"
    build_root = DEBMAGIC_TEMP_BUILD_PARENT_DIR / package_identifier
    if build_root.exists():
        shutil.rmtree(build_root)

    config = BuildConfig(
        package_identifier=package_identifier,
        source_dir=source_dir,
        output_dir=output_dir,
        build_root_dir=build_root,
        distro="debian",
        distro_version="trixie",
        dry_run=dry_run,
        sign_package=False,
    )

    # prepare build environment, create the build directory structure, copy the sources
    config.create_dirs()
    source_ignore_pattern = _ignore_patterns_from_gitignore(source_dir / ".gitignore")
    shutil.copytree(config.source_dir, config.build_source_dir, dirs_exist_ok=True, ignore=source_ignore_pattern)

    return config


def _copy_file_if_exists(source: Path, glob: str, dest: Path):
    for file in source.glob(glob):
        if file.is_dir():
            shutil.copytree(file, dest)
        elif file.is_file():
            shutil.copy(file, dest)
        else:
            raise NotImplementedError("Don't support anything besides files and directories")


def build(build_driver: BuildDriverType, source_dir: Path, output_dir: Path, dry_run: bool = False):
    config = _prepare_build_env(source_dir=source_dir, output_dir=output_dir, dry_run=dry_run)

    driver = _create_driver(build_driver, config)
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
        _copy_file_if_exists(source=config.build_source_dir / "..", glob="*.deb", dest=config.output_dir)
        _copy_file_if_exists(source=config.build_source_dir / "..", glob="*.buildinfo", dest=config.output_dir)
        _copy_file_if_exists(source=config.build_source_dir / "..", glob="*.changes", dest=config.output_dir)
        _copy_file_if_exists(source=config.build_source_dir / "..", glob="*.dsc", dest=config.output_dir)
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
