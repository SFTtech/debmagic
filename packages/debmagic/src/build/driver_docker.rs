use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::build::common::{
    BuildConfig, BuildDriver, BuildDriverType, BuildMetadata, DriverSpecificBuildMetadata,
};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
    _driver_config: DriverDockerConfig,
    container_name: String,
}

impl DriverDocker {
    pub fn create(
        config: &BuildConfig,
        driver_config: &DriverDockerConfig,
    ) -> anyhow::Result<Self> {
        let base_image = driver_config
            .base_image
            .clone()
            .unwrap_or_else(|| format!("docker.io/{}:{}", config.distro, config.distro_version));

        let formatted_dockerfile = DOCKERFILE_TEMPLATE
            .replace("{base_image}", &base_image)
            .replace("{docker_user}", DOCKER_USER)
            .replace("{build_dir}", BUILD_DIR_IN_CONTAINER)
            .replace(
                "{debian_control_file}",
                &config
                    .build_source_dir()
                    .join("debian")
                    .join("control")
                    .to_string_lossy(),
            );

        let dockerfile_path = config.build_temp_dir().join("Dockerfile");
        fs::write(&dockerfile_path, formatted_dockerfile).expect("Failed to write Dockerfile");

        fs::create_dir_all(config.build_temp_dir().join("debian"))?;
        fs::copy(
            config.build_source_dir().join("debian").join("control"),
            config.build_temp_dir().join("debian").join("control"),
        )?;

        let docker_image_name = format!("debmagic-{}", config.build_identifier());
        let mut build_args = Vec::new();

        let uid = unsafe { libc::getuid() };
        if uid != 0 {
            build_args.extend(["--build-arg".to_string(), format!("USER_UID={uid}")]);
        }
        let gid = unsafe { libc::getgid() };
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

        if !config.dry_run && !build_cmd.status().map(|s| s.success()).unwrap_or(false) {
            return Err(anyhow!("Error creating docker image"));
        }

        let container_uuid = Uuid::new_v4().to_string();
        let mut run_cmd = Command::new("docker");
        run_cmd.args([
            "run",
            "--detach",
            "--name",
            &container_uuid,
            "--mount",
            &format!(
                "type=bind,src={},dst={}",
                config.build_root_dir.display(),
                BUILD_DIR_IN_CONTAINER
            ),
            &docker_image_name,
        ]);

        if !config.dry_run && !run_cmd.status().map(|s| s.success()).unwrap_or(false) {
            return Err(anyhow!("Error starting docker container"));
        }

        Ok(Self {
            config: config.clone(),
            _driver_config: driver_config.clone(),
            container_name: container_uuid,
        })
    }

    pub fn from_build_metadata(
        config: &BuildConfig,
        driver_config: &DriverDockerConfig,
        build_metadata: &BuildMetadata,
    ) -> Self {
        let container_name = build_metadata
            .driver_metadata
            .get("container_name")
            .cloned()
            .expect("Missing container_name in metadata");

        Self {
            config: config.clone(),
            _driver_config: driver_config.clone(),
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

        if self.config.dry_run {
            println!("[dry-run] docker {exec_args:?}");
            return Ok(());
        }

        let status = Command::new("docker").args(exec_args).status()?;
        if !status.success() {
            return Err(std::io::Error::other("Docker exec failed"));
        }
        Ok(())
    }

    fn cleanup(&self) {
        let _ = Command::new("docker")
            .args(["rm", "-f", &self.container_name])
            .status();
    }

    fn drop_into_shell(&self) -> std::io::Result<()> {
        if self.config.dry_run {
            return Ok(());
        }

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
