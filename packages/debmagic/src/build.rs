use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    build::{
        common::{BuildConfig, BuildDriver, BuildDriverType, BuildMetadata, PackageDescription},
        config::DriverConfig,
        driver_bare::DriverBare,
        driver_docker::DriverDocker,
    },
    config::Config,
};
use anyhow::{Context, anyhow};
use glob::glob;

pub mod common;
pub mod config;
pub mod driver_bare;
pub mod driver_docker;

fn create_driver(
    driver_type: &BuildDriverType,
    build_config: &BuildConfig,
    config: &DriverConfig,
) -> anyhow::Result<Box<dyn BuildDriver>> {
    match driver_type {
        BuildDriverType::Docker => Ok(Box::new(DriverDocker::create(
            build_config,
            &config.docker,
        )?)),
        BuildDriverType::Bare => Ok(Box::new(DriverBare::create(build_config, &config.bare))),
        // BuildDriverType::Lxd => ...
    }
}

fn create_driver_from_build_root(
    config: &DriverConfig,
    build_root: &Path,
) -> anyhow::Result<Box<dyn BuildDriver>> {
    let build_metadata_path = build_root.join("build.json");
    if !build_metadata_path.is_file() {
        return Err(anyhow!("No build.json found"));
    }

    let content = fs::read_to_string(&build_metadata_path)?;
    let metadata: BuildMetadata = serde_json::from_str(&content).with_context(|| {
        format!(
            "Failed to read build metadata from {} - invalid json",
            build_metadata_path.display()
        )
    })?;

    match metadata.driver {
        BuildDriverType::Docker => Ok(Box::new(DriverDocker::from_build_metadata(
            &metadata.config,
            &config.docker,
            &metadata,
        ))),
        BuildDriverType::Bare => Ok(Box::new(DriverBare::from_build_metadata(
            &metadata.config,
            &config.bare,
            &metadata,
        ))),
        // BuildDriverType::Lxd => ...
    }
}

fn write_build_metadata(config: &BuildConfig, driver: &dyn BuildDriver) -> anyhow::Result<()> {
    let metadata = BuildMetadata {
        driver: driver.driver_type(),
        config: config.clone(),
        driver_metadata: driver.get_build_metadata(),
    };
    let path = config.build_root_dir.join("build.json");
    let json =
        serde_json::to_string_pretty(&metadata).context("Failed serialize build metadata")?;
    fs::write(path, json)?;
    Ok(())
}

fn copy_glob(src_dir: &Path, pattern: &str, dest_dir: &Path) -> anyhow::Result<()> {
    let full_pattern = src_dir.join(pattern).to_string_lossy().into_owned();
    for entry in glob(&full_pattern)? {
        let path = entry?;
        if path.is_file() {
            let filename = path.file_name().ok_or(anyhow!(
                "Could not retrieve filename from {}",
                path.display()
            ))?;
            fs::copy(&path, dest_dir.join(filename))?;
        }
    }
    Ok(())
}

fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> std::io::Result<()> {
    // TODO: properly handle gitignore / other ignore files when copying
    // Use ignore crate
    // Simple copy logic (for advanced gitignore support, look at the `ignore` crate)
    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
        // handle hardlinks, symlinks and similar weird filetypes
    }
    Ok(())
}

fn get_build_root_and_identifier(
    config: &Config,
    package: &PackageDescription,
) -> (String, PathBuf) {
    let package_identifier = format!("{}-{}", package.name, package.version);
    let build_root = config.temp_build_dir.join(&package_identifier);
    (package_identifier, build_root)
}

fn prepare_build_env(
    config: &Config,
    package: &PackageDescription,
    output_dir: &Path,
) -> anyhow::Result<BuildConfig> {
    let (package_identifier, build_root) = get_build_root_and_identifier(config, package);
    if build_root.exists() {
        fs::remove_dir_all(&build_root)?;
    }

    let build_config = BuildConfig {
        package_identifier,
        source_dir: package.source_dir.clone(),
        output_dir: output_dir.to_path_buf(),
        build_root_dir: build_root,
        distro: "debian".to_string(),
        distro_version: "trixie".to_string(),
        dry_run: config.dry_run,
        sign_package: false,
    };

    build_config.create_dirs()?;

    copy_dir_all(&build_config.source_dir, build_config.build_source_dir())?;

    Ok(build_config)
}

pub fn get_shell_in_build(config: &Config, package: &PackageDescription) -> anyhow::Result<()> {
    let (_package_identifier, build_root) = get_build_root_and_identifier(config, package);
    let driver = create_driver_from_build_root(&config.driver, &build_root)?;
    driver.drop_into_shell()?;
    Ok(())
}

pub fn build_package(
    config: &Config,
    package: &PackageDescription,
    driver_type: BuildDriverType,
    output_dir: &Path,
) -> anyhow::Result<()> {
    let build_config = prepare_build_env(config, package, output_dir)?;

    let driver = create_driver(&driver_type, &build_config, &config.driver)?;

    write_build_metadata(&build_config, &*driver)?;

    let result = (|| -> anyhow::Result<()> {
        driver.run_command(
            &["apt-get", "-y", "build-dep", "."],
            &build_config.build_source_dir(),
            true,
        )?;
        driver.run_command(
            &["dpkg-buildpackage", "-us", "-uc", "-ui", "-nc", "-b"],
            &build_config.build_source_dir(),
            false,
        )?;

        if build_config.sign_package {
            // SIGN .changes and .dsc files
            // changes = *.changes / *.dsc
            // driver.run_command(&["debsign", opts, changes], &build_config.build_source_dir(), false)?;
            // driver.run_command(&["debrsign", opts, username, changes],  &build_config.build_source_dir(), false)?;
        }

        let parent_dir = build_config.build_source_dir().join("..");
        copy_glob(&parent_dir, "*.deb", &build_config.output_dir)?;
        copy_glob(&parent_dir, "*.changes", &build_config.output_dir)?;
        copy_glob(&parent_dir, "*.buildinfo", &build_config.output_dir)?;
        copy_glob(&parent_dir, "*.dsc", &build_config.output_dir)?;

        Ok(())
    })();

    if let Err(e) = result {
        eprintln!("Build failed: {e}. Dropping into shell...");
        let res = driver.drop_into_shell();
        if let Err(shell_error) = res {
            eprintln!("Dropping into shell failed: {shell_error}");
        }
        driver.cleanup();
        return Err(e);
    }

    driver.cleanup();
    Ok(())
}
