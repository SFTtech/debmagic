use core::time;
use std::{
    cmp::{self},
    fs,
    io::{self, BufReader, BufWriter, IsTerminal, Seek, stdout},
    path::{Path, PathBuf},
    thread,
};

use crate::{
    build::{
        common::{BuildConfig, BuildDriver, BuildDriverType, BuildMetadata},
        config::DriverConfig,
        driver_bare::DriverBare,
        driver_docker::DriverDocker,
    },
    config::Config,
    package::PackageDescription,
};
use anyhow::{Context, anyhow};
use glob::glob;

pub mod common;
pub mod config;
pub mod driver_bare;
pub mod driver_docker;

struct Build {
    config: BuildConfig,
    pub driver: Box<dyn BuildDriver>,
}

fn get_build_driver(
    config: &BuildConfig,
    driver_config: &DriverConfig,
) -> anyhow::Result<Box<dyn BuildDriver>> {
    match config.driver {
        BuildDriverType::Docker => Ok(Box::new(DriverDocker::create(config, driver_config)?)),
        BuildDriverType::Bare => Ok(Box::new(DriverBare::create(config, driver_config))),
        // BuildDriverType::Lxd => ...
    }
}

fn create_driver_from_metadata(
    config: &DriverConfig,
    metadata: &BuildMetadata,
) -> anyhow::Result<Box<dyn BuildDriver>> {
    let driver: anyhow::Result<Box<dyn BuildDriver>> = match &metadata.config.driver {
        BuildDriverType::Docker => Ok(Box::new(DriverDocker::from_build_metadata(
            &metadata.config,
            config,
            metadata,
        ))),
        BuildDriverType::Bare => Ok(Box::new(DriverBare::from_build_metadata(
            &metadata.config,
            config,
            metadata,
        ))),
        // BuildDriverType::Lxd => ...
    };
    driver
}

impl Build {
    pub fn create(config: &BuildConfig, driver_config: &DriverConfig) -> anyhow::Result<Self> {
        let driver = get_build_driver(config, driver_config)
            .context(format!("failed to create {:?} build driver", config.driver))?;
        Ok(Self {
            config: config.clone(),
            driver,
        })
    }

    pub fn from_build_root(
        build_root: &Path,
        driver_config: &DriverConfig,
    ) -> anyhow::Result<Self> {
        let build_metadata_path = build_root.join("build.json");
        if !build_metadata_path.is_file() {
            return Err(anyhow!("No build.json found"));
        }

        let mut file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&build_metadata_path)?;
        file.lock()?;

        let metadata = || -> anyhow::Result<BuildMetadata> {
            let reader = BufReader::new(&file);
            let metadata: BuildMetadata = serde_json::from_reader(reader).with_context(|| {
                format!(
                    "Failed to read build metadata from {} - invalid json",
                    build_metadata_path.display()
                )
            })?;
            Ok(metadata)
        }();

        let metadata = match metadata {
            Err(meta_err) => {
                file.unlock()?;
                return Err(meta_err);
            }
            Ok(metadata) => metadata,
        };

        let driver = create_driver_from_metadata(driver_config, &metadata);

        let driver = match driver {
            Err(driver_err) => {
                file.unlock()?;
                return Err(driver_err);
            }
            Ok(driver) => driver,
        };

        let result = || -> anyhow::Result<()> {
            file.seek(io::SeekFrom::Start(0))?;
            let updated_metadata = BuildMetadata {
                num_processes_attached: &metadata.num_processes_attached + 1,
                ..metadata.clone()
            };
            let writer = BufWriter::new(&file);
            serde_json::to_writer_pretty(writer, &updated_metadata)
                .context("Failed to serialize build metadata")?;
            Ok(())
        }();

        file.unlock()?;

        result?;

        Ok(Self {
            config: metadata.config.clone(),
            driver,
        })
    }

    fn build_metadata_path(&self) -> anyhow::Result<PathBuf> {
        let build_metadata_path = self.config.build_root_dir.join("build.json");
        if !build_metadata_path.is_file() {
            return Err(anyhow!("No build.json found"));
        }
        Ok(build_metadata_path)
    }

    pub fn detach(&self) -> anyhow::Result<()> {
        // TODO: refactor the whole file locking / read + write metadata thing to not be as duplicated and error prone
        let build_metadata_path = self.build_metadata_path()?;

        let mut file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&build_metadata_path)?;
        file.lock()?;

        let metadata = || -> anyhow::Result<BuildMetadata> {
            let reader = BufReader::new(&file);
            let metadata: BuildMetadata = serde_json::from_reader(reader).with_context(|| {
                format!(
                    "Failed to read build metadata from {} - invalid json",
                    build_metadata_path.display()
                )
            })?;
            Ok(metadata)
        }();

        let metadata = match metadata {
            Err(meta_err) => {
                file.unlock()?;
                return Err(meta_err);
            }
            Ok(metadata) => metadata,
        };

        let result = || -> anyhow::Result<()> {
            file.seek(io::SeekFrom::Start(0))?;
            let updated_metadata = BuildMetadata {
                num_processes_attached: cmp::max(0, &metadata.num_processes_attached - 1),
                ..metadata.clone()
            };
            let writer = BufWriter::new(&file);
            serde_json::to_writer_pretty(writer, &updated_metadata)
                .context("Failed to serialize build metadata")?;
            Ok(())
        }();

        file.unlock()?;
        result
    }

    pub fn get_number_of_attached_processes(&self) -> anyhow::Result<i64> {
        // TODO: refactor the whole file locking / read + write metadata thing to not be as duplicated and error prone
        let build_metadata_path = self.build_metadata_path()?;

        let file = fs::OpenOptions::new()
            .read(true)
            .open(&build_metadata_path)?;
        file.lock()?;

        let metadata = || -> anyhow::Result<BuildMetadata> {
            let reader = BufReader::new(&file);
            let metadata: BuildMetadata = serde_json::from_reader(reader).with_context(|| {
                format!(
                    "Failed to read build metadata from {} - invalid json",
                    build_metadata_path.display()
                )
            })?;
            Ok(metadata)
        }();
        file.unlock()?;

        match metadata {
            Err(meta_err) => Err(meta_err),
            Ok(metadata) => Ok(metadata.num_processes_attached),
        }
    }

    pub fn write_metadata(&self) -> anyhow::Result<()> {
        let metadata = BuildMetadata {
            config: self.config.clone(),
            driver_metadata: self.driver.get_build_metadata(),
            num_processes_attached: 0,
        };
        let path = self.config.build_root_dir.join("build.json");
        let json = serde_json::to_string_pretty(&metadata)
            .context("Failed to serialize build metadata")?;
        fs::write(path, json)?;
        Ok(())
    }
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

fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> anyhow::Result<()> {
    fs::create_dir_all(&dst)?;

    let walker = ignore::WalkBuilder::new(&src)
        .standard_filters(true)
        .hidden(false)
        .filter_entry(|entry| !(entry.path().is_dir() && entry.path().ends_with(".git")))
        .build();

    for entry in walker {
        let entry = entry?;
        let file_type = entry.file_type().ok_or(anyhow!(
            "failed to get file type of {}",
            entry.path().display()
        ))?;

        // get path of entry relative to src
        let relative_path = entry
            .path()
            .strip_prefix(src.as_ref())
            .context("failed to get relative path")?;

        if file_type.is_dir() {
            fs::create_dir_all(dst.as_ref().join(relative_path))?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), dst.as_ref().join(relative_path))
                .context(format!("failed to copy file: {}", entry.path().display()))?;
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
    driver_type: BuildDriverType,
    output_dir: &Path,
) -> anyhow::Result<Build> {
    let (package_identifier, build_root) = get_build_root_and_identifier(config, package);
    if build_root.exists() {
        fs::remove_dir_all(&build_root)?;
    }

    let build_config = BuildConfig {
        driver: driver_type,
        package_identifier,
        source_dir: package.source_dir.clone(),
        output_dir: output_dir.to_path_buf(),
        build_root_dir: build_root,
        distro: "debian".to_string(),
        distro_version: "forky".to_string(),
        sign_package: false,
    };

    build_config
        .create_dirs()
        .context("failed to create build directories")?;

    copy_dir_all(&build_config.source_dir, build_config.build_source_dir())
        .context("failed to copy source tree to build directory")?;

    let build = Build::create(&build_config, &config.driver)?;
    Ok(build)
}

pub fn get_shell_in_build(config: &Config, package: &PackageDescription) -> anyhow::Result<()> {
    let (_package_identifier, build_root) = get_build_root_and_identifier(config, package);
    let build = Build::from_build_root(&build_root, &config.driver)?;
    let result = build.driver.interactive_shell();

    // TODO: detach - decrement num_attached_processes
    build.detach()?;

    result?;
    Ok(())
}

pub fn build_package(
    config: &Config,
    package: &PackageDescription,
    driver_type: BuildDriverType,
    output_dir: &Path,
) -> anyhow::Result<()> {
    let build = prepare_build_env(config, package, driver_type, output_dir)
        .context("failed to prepare build environment")?;
    build
        .write_metadata()
        .context("failed to write build metadata")?;

    let result = (|| -> anyhow::Result<()> {
        build.driver.run_command(
            &["apt-get", "-y", "build-dep", "."],
            &build.config.build_source_dir(),
            true,
        )?;
        build.driver.run_command(
            &["dpkg-buildpackage", "-us", "-uc", "-ui", "-nc", "-b"],
            &build.config.build_source_dir(),
            false,
        )?;

        if build.config.sign_package {
            // SIGN .changes and .dsc files
            // changes = *.changes / *.dsc
            // driver.run_command(&["debsign", opts, changes], &build_config.build_source_dir(), false)?;
            // driver.run_command(&["debrsign", opts, username, changes],  &build_config.build_source_dir(), false)?;
        }

        let parent_dir = build.config.build_source_dir().join("..");
        copy_glob(&parent_dir, "*.deb", &build.config.output_dir)?;
        copy_glob(&parent_dir, "*.changes", &build.config.output_dir)?;
        copy_glob(&parent_dir, "*.buildinfo", &build.config.output_dir)?;
        copy_glob(&parent_dir, "*.dsc", &build.config.output_dir)?;

        Ok(())
    })();

    if let Err(e) = result {
        if stdout().is_terminal() {
            eprintln!("Build failed: {e}. Dropping into shell...");
            let res = build.driver.interactive_shell();
            if let Err(shell_error) = res {
                eprintln!("Dropping into shell failed: {shell_error}");
            }
        } else {
            eprintln!("Build failed: {e}");
        }
        build.driver.cleanup();
        return Err(e);
    }

    // busy waiting until no pro
    while let Ok(attached_processes) = build.get_number_of_attached_processes()
        && attached_processes > 0
    {
        println!("Waiting for last shell to detach ...");
        thread::sleep(time::Duration::from_millis(10));
    }

    build.driver.cleanup();
    Ok(())
}
