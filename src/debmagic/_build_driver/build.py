from debmagic._build_driver.driver_docker import BuildDriverDocker
from debmagic._build_driver.driver_lxd import BuildDriverLxd
from debmagic._build_driver.driver_none import BuildDriverNone

from .common import BuildConfig, BuildDriver, BuildDriverType


def _create_driver(build_driver: BuildDriverType, config: BuildConfig) -> BuildDriver:
    match build_driver:
        case "docker":
            return BuildDriverDocker.create(config=config)
        case "lxd":
            return BuildDriverLxd.create(config=config)
        case "none":
            return BuildDriverNone.create(config=config)


def build(build_driver: BuildDriverType, config: BuildConfig):
    driver = _create_driver(build_driver, config)
    try:
        driver.run_command(["apt-get", "-y", "build-dep", "."], cwd=config.source_dir)
        driver.run_command(["debuild", "-nc", "-uc", "-b"], cwd=config.source_dir)

        # TODO: copy packages to output directory
        config.output_dir.mkdir(parents=True, exist_ok=True)
        driver.copy_file(source_dir=config.source_dir / "..", glob="debmagic_*.deb", dest_dir=config.output_dir)
    except Exception as e:
        print(e)
        print(
            "Something failed during building - dropping into interactive shell in build environment for easier debugging"
        )
        driver.drop_into_shell()
        raise e
    finally:
        driver.cleanup()
