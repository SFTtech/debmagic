use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::anyhow;
use serde::{Deserialize, Serialize};

use crate::build::{
    common::{
        BuildConfig, BuildDriver, BuildDriverType, BuildMetadata, DriverSpecificBuildMetadata,
    },
    config::DriverConfig,
};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DriverDockerConfig {
    pub base_image: Option<String>,
}

// Constants
const BUILD_DIR_IN_CONTAINER: &str = "/debmagic";
const DOCKER_USER: &str = "user";
const DOCKERFILE_TEMPLATE: &str = r#"
FROM {base_image}
ARG USERNAME={docker_user}
ARG USER_UID=1000
ARG USER_GID=$USER_UID
RUN apt-get update && apt-get install -y sudo dpkg-dev python3
RUN groupadd --gid $USER_GID $USERNAME \
    && useradd --uid $USER_UID --gid $USER_GID -m $USERNAME \
    && echo $USERNAME ALL=\(root\) NOPASSWD:ALL > /etc/sudoers.d/$USERNAME \
    && chmod 0440 /etc/sudoers.d/$USERNAME
RUN mkdir -p /build/package/debian
RUN --mount=type=bind,source=debian/control,target=/build/package/debian/control apt-get -y build-dep /build/package
RUN mkdir -p {build_dir}
RUN chown $USERNAME:$USERNAME {build_dir}
USER $USERNAME
ENTRYPOINT ["sleep", "infinity"]
"#;

#[derive(Debug, Serialize, Deserialize)]
pub struct DockerDriverBuildMetadata {
    pub container_name: String,
}

pub struct DriverDocker {
    config: BuildConfig,
    driver_config: DriverConfig,
    container_name: String,
}

fn error_from_command(cmd: &mut Command, message: &str) -> anyhow::Error {
    anyhow!(
        "{message}:\n{}",
        cmd.output()
            .map(|o| String::from_utf8_lossy(&o.stderr).to_string())
            .unwrap_or("".to_string())
    )
}

fn build_build_image(config: &BuildConfig, driver_config: &DriverConfig) -> anyhow::Result<String> {
    let base_image = driver_config
        .docker
        .base_image
        .clone()
        .unwrap_or_else(|| format!("docker.io/{}:{}", config.distro, config.distro_version));

    let debian_control_file_path = config.build_source_dir().join("debian").join("control");

    let formatted_dockerfile = DOCKERFILE_TEMPLATE
        .replace("{base_image}", &base_image)
        .replace("{docker_user}", DOCKER_USER)
        .replace("{build_dir}", BUILD_DIR_IN_CONTAINER)
        .replace(
            "{debian_control_file}",
            &debian_control_file_path.to_string_lossy(),
        );

    let dockerfile_path = config.build_temp_dir().join("Dockerfile");
    fs::write(&dockerfile_path, formatted_dockerfile)
        .map_err(|e| anyhow!("Failed to write Dockerfile, {e}"))?;

    fs::create_dir_all(config.build_temp_dir().join("debian"))
        .map_err(|e| anyhow!("Failed to create debian directory: {e}"))?;
    fs::copy(
        &debian_control_file_path,
        config.build_temp_dir().join("debian").join("control"),
    )
    .map_err(|e| anyhow!("Failed to copy debian control file: {e}"))?;

    let docker_image_name = format!("debmagic-{}", config.build_identifier());
    let mut build_args = Vec::new();

    let uid = unsafe { libc::geteuid() };
    if uid != 0 {
        build_args.extend(["--build-arg".to_string(), format!("USER_UID={uid}")]);
    }
    let gid = unsafe { libc::getegid() };
    if gid != 0 {
        build_args.extend(["--build-arg".to_string(), format!("USER_GID={gid}")]);
    }

    let mut build_cmd = Command::new("docker");
    build_cmd.args(["build"]).args(&build_args).args([
        "--tag",
        &docker_image_name,
        "-f",
        &dockerfile_path.to_string_lossy(),
        &config.build_temp_dir().to_string_lossy(),
    ]);

    if !build_cmd.status().map(|s| s.success()).unwrap_or(false) {
        return Err(error_from_command(
            &mut build_cmd,
            "Error creating docker image",
        ));
    }

    Ok(docker_image_name)
}

fn does_container_exist(container_name: &str) -> anyhow::Result<bool> {
    let mut ps_cmd = Command::new("docker");
    ps_cmd.args(["ps", "--all", "--format", "json"]);
    ps_cmd.stdout(std::process::Stdio::piped());
    if !ps_cmd.status().map(|s| s.success()).unwrap_or(false) {
        return Err(anyhow!("failed to query running docker containers"));
    }

    let output = ps_cmd
        .output()
        .map_err(|_| anyhow!("Failed to read docker ps output"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Ok(container) = serde_json::from_str::<serde_json::Value>(line)
            && let Some(names) = container.get("Names")
            && names == container_name
        {
            return Ok(true);
        }
    }
    Ok(false)
}

impl DriverDocker {
    pub fn create(config: &BuildConfig, driver_config: &DriverConfig) -> anyhow::Result<Self> {
        let container_name = format!("debmagic-{}", config.build_identifier());
        let container_exists = does_container_exist(&container_name)?;

        if driver_config.persistent && container_exists {
            let mut start_cmd = Command::new("docker");
            start_cmd.args(["start", &container_name]);
            if !start_cmd.status().map(|s| s.success()).unwrap_or(false) {
                return Err(error_from_command(
                    &mut start_cmd,
                    "Error starting docker container",
                ));
            }
        } else {
            if container_exists {
                let mut rm_cmd = Command::new("docker");
                rm_cmd.args(["rm", "-f", &container_name]);
                if !rm_cmd.status().map(|s| s.success()).unwrap_or(false) {
                    return Err(error_from_command(
                        &mut rm_cmd,
                        "Error removing existing docker container",
                    ));
                }
            }

            let docker_image_name = build_build_image(config, driver_config)?;
            let mut run_cmd = Command::new("docker");
            run_cmd.args([
                "run",
                "--detach",
                "--name",
                &container_name,
                "--mount",
                &format!(
                    "type=bind,src={},dst={}",
                    config.build_root_dir.display(),
                    BUILD_DIR_IN_CONTAINER
                ),
                &docker_image_name,
            ]);

            if !run_cmd.status().map(|s| s.success()).unwrap_or(false) {
                return Err(error_from_command(
                    &mut run_cmd,
                    "Error starting docker container",
                ));
            }
        }

        Ok(Self {
            config: config.clone(),
            driver_config: driver_config.clone(),
            container_name,
        })
    }

    pub fn from_build_metadata(
        config: &BuildConfig,
        driver_config: &DriverConfig,
        build_metadata: &BuildMetadata,
    ) -> Self {
        let container_name = build_metadata
            .driver_metadata
            .get("container_name")
            .cloned()
            .expect("Missing container_name in metadata");

        Self {
            config: config.clone(),
            driver_config: driver_config.clone(),
            container_name,
        }
    }
    fn translate_path_in_container(
        &self,
        path_in_source: &Path,
    ) -> Result<PathBuf, std::io::Error> {
        path_in_source
            .strip_prefix(&self.config.build_root_dir)
            .map(|rel| Path::new(BUILD_DIR_IN_CONTAINER).join(rel))
            .map_err(|_| {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Path is not relative to build root".to_string(),
                )
            })
    }
}

impl BuildDriver for DriverDocker {
    fn get_build_metadata(&self) -> DriverSpecificBuildMetadata {
        let mut meta = DriverSpecificBuildMetadata::new();
        meta.insert("container_name".to_string(), self.container_name.clone());
        meta
    }

    fn run_command(&self, cmd: &[&str], cwd: &Path, requires_root: bool) -> std::io::Result<()> {
        let mut exec_args = vec!["exec".to_string()];

        let container_path = self
            .translate_path_in_container(cwd)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
        exec_args.push("--workdir".to_string());
        exec_args.push(container_path.to_string_lossy().to_string());

        if requires_root {
            exec_args.push("--user".to_string());
            exec_args.push("root".to_string());
        }

        exec_args.push(self.container_name.clone());
        exec_args.extend(cmd.iter().map(|s| s.to_string()));

        let status = Command::new("docker").args(exec_args).status()?;
        if !status.success() {
            return Err(std::io::Error::other("Docker exec failed"));
        }
        Ok(())
    }

    fn cleanup(&self) {
        if self.driver_config.persistent {
            let _ = Command::new("docker")
                .args(["stop", &self.container_name])
                .status();
        } else {
            let _ = Command::new("docker")
                .args(["rm", "-f", &self.container_name])
                .status();
        }
    }

    fn interactive_shell(&self) -> std::io::Result<()> {
        let workdir = self.translate_path_in_container(&self.config.build_root_dir)?;
        let _ = Command::new("docker")
            .args([
                "exec",
                "-it",
                "--workdir",
                &workdir.to_string_lossy(),
                &self.container_name,
                "/usr/bin/env",
                "bash",
            ])
            .status()?;

        Ok(())
    }

    fn driver_type(&self) -> BuildDriverType {
        BuildDriverType::Docker
    }
}
