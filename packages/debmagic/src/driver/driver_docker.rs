use std::{
    collections::HashMap,
    fs,
    io::IsTerminal,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::SystemTime,
};

use anyhow::{Context, anyhow};
use debmagic_common::{apt::Archives, distro::DistroVersion};
use serde::{Deserialize, Serialize};

use crate::apt;
use crate::driver::{
    CommandOptions, DriverType, ENVIRONMENT_DIR_IN_CONTAINER, Environment, EnvironmentDriver,
    EnvironmentMetadata, EnvironmentPurpose, IsolationCapability, ResourceStatus,
    config::DriverConfig, container_name_from_metadata, container_name_metadata,
    environment_fingerprint, getent_passwd, resource_name, run_checked,
    translate_path_in_container,
};
use crate::subprocess::{self, CommandResult};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DriverDockerConfig {
    pub base_images: HashMap<String, String>,
}

impl DriverDockerConfig {
    pub fn base_image_for_distro(&self, distro: &DistroVersion) -> String {
        self.base_images
            .get(&format!("{}:{}", distro.distro, distro.codename))
            .cloned()
            .unwrap_or_else(|| format!("docker.io/{}:{}", distro.distro, distro.codename))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverDockerConfigOverrides {
    pub base_image: Option<String>,
}

// Constants
const ENVIRONMENT_LABEL: &str = "dev.debmagic.environment";

fn bind_mount_arg(src: &Path, dst: &str) -> String {
    format!("type=bind,src={},dst={}", src.display(), dst)
}
const DOCKERFILE_TEMPLATE: &str = r#"
FROM {base_image}
ARG USER_UID=1000
ARG USER_GID=$USER_UID
ARG DEBIAN_FRONTEND=noninteractive
COPY {setup_dir} /tmp/{setup_dir}
RUN {setup} && rm -rf /tmp/{setup_dir} && {apt}
RUN set -e; \
    getent group "$USER_GID" >/dev/null || groupadd --gid "$USER_GID" debmagic; \
    getent passwd "$USER_UID" >/dev/null || useradd --uid "$USER_UID" --gid "$USER_GID" -m debmagic
RUN mkdir -p {build_dir} && chown $USER_UID:$USER_GID {build_dir}
USER $USER_UID:$USER_GID
ENTRYPOINT ["sleep", "infinity"]
"#;

/// The image applies the apt setup and refreshes its packages, so a fresh
/// container from a cached image is as current as the image.
fn dockerfile(environment: &Environment, base_image: &str, setup: &apt::Setup) -> String {
    let setup_dir = apt::SETUP_DIR;
    let mut apt_commands = apt::refresh_commands();
    apt_commands.extend(apt::install_commands(
        debmagic_common::apt::has_satisfy(&environment.distro),
        &apt::Install::default(),
        None,
    ));
    DOCKERFILE_TEMPLATE
        .replace("{base_image}", base_image)
        .replace("{setup}", &setup.script(&format!("/tmp/{setup_dir}")))
        .replace("{apt}", &apt::shell_line(&apt_commands))
        .replace("{setup_dir}", setup_dir)
        .replace("{build_dir}", ENVIRONMENT_DIR_IN_CONTAINER)
}

pub struct DriverDocker {
    environment: Environment,
    container_name: String,
    reused_environment: bool,
    /// Creation time of the image a fresh container was started from.
    image_created: Option<SystemTime>,
}

/// Replace characters that Docker image/container names disallow.
fn sanitize_docker_reference(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '_' | '-' => c,
            _ => '-',
        })
        .collect()
}

fn build_build_image(
    environment: &Environment,
    dockerfile: &str,
    setup: &apt::Setup,
    image_name: &str,
) -> anyhow::Result<()> {
    setup
        .write(&environment.temp_dir().join(apt::SETUP_DIR))
        .context("writing the apt setup failed")?;
    let dockerfile_path = environment.temp_dir().join("Dockerfile");
    fs::write(&dockerfile_path, dockerfile)
        .map_err(|e| anyhow!("Failed to write Dockerfile, {e}"))?;

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
    build_cmd
        .args(["build"])
        .args(&build_args)
        .args(["--tag", image_name, "-f"])
        .arg(dockerfile_path)
        .arg(environment.temp_dir());

    run_checked(&mut build_cmd, "building docker image")?;

    Ok(())
}

/// The image's creation time, `None` if it doesn't exist.
fn image_created(image_name: &str) -> anyhow::Result<Option<SystemTime>> {
    let output = Command::new("docker")
        .args(["image", "inspect", "--format", "{{.Created}}", image_name])
        .stderr(Stdio::null())
        .output()
        .map_err(|error| anyhow!("Failed to inspect Docker image: {error}"))?;
    if !output.status.success() {
        return Ok(None);
    }
    let created = String::from_utf8_lossy(&output.stdout);
    let created = chrono::DateTime::parse_from_rfc3339(created.trim())
        .with_context(|| format!("parsing the creation time {created:?} of {image_name} failed"))?;
    Ok(Some(created.into()))
}

/// The environment fingerprint stored in the container's labels, or `None`
/// if the container does not exist (or has no fingerprint label).
fn container_environment_fingerprint(container_name: &str) -> anyhow::Result<Option<String>> {
    let output = Command::new("docker")
        .args(["inspect", container_name])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .map_err(|error| anyhow!("failed to inspect Docker container: {error}"))?;
    if !output.status.success() {
        // docker inspect exits non-zero when the container doesn't exist.
        return Ok(None);
    }
    let containers: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout)
        .map_err(|error| anyhow!("failed to parse Docker inspect output: {error}"))?;
    Ok(containers
        .first()
        .and_then(|container| container.pointer("/Config/Labels"))
        .and_then(|labels| labels.get(ENVIRONMENT_LABEL))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned))
}

impl DriverDocker {
    fn container_start(&self) -> anyhow::Result<()> {
        run_checked(
            Command::new("docker").args(["start", &self.container_name]),
            "starting docker container",
        )
    }

    fn container_stop(&self) -> anyhow::Result<()> {
        run_checked(
            Command::new("docker").args(["stop", &self.container_name]),
            "stopping docker container",
        )
    }

    fn container_is_running(&self) -> anyhow::Result<bool> {
        let output = Command::new("docker")
            .args([
                "inspect",
                "--format",
                "{{.State.Running}}",
                &self.container_name,
            ])
            .output()
            .map_err(|error| anyhow!("failed to inspect Docker container: {error}"))?;
        if !output.status.success() {
            return Err(anyhow!(
                "Docker container {} not found",
                self.container_name
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim() == "true")
    }

    fn container_remove_force(&self) -> anyhow::Result<()> {
        run_checked(
            Command::new("docker").args(["rm", "-f", &self.container_name]),
            "removing docker container",
        )
    }

    /// Whether `/debmagic` in the container still resolves to the current
    /// build-root inode on the host.
    fn mount_is_live(&self) -> anyhow::Result<bool> {
        let container_probe = crate::driver::write_mount_probe(&self.environment.root_dir)?;
        let output = self.run_command(
            &["test", "-f", &container_probe.to_string_lossy()],
            &self.environment.root_dir,
            CommandOptions {
                requires_root: true,
                ..Default::default()
            },
        );
        crate::driver::remove_mount_probe(&self.environment.root_dir);
        Ok(output?.exit_code == 0)
    }

    fn container_name_for(environment: &Environment) -> String {
        resource_name(
            "debmagic",
            &environment.package_name,
            &sanitize_docker_reference(&environment.id()),
        )
    }

    /// Driver metadata known before the container exists.
    pub fn planned_metadata(environment: &Environment) -> HashMap<String, String> {
        container_name_metadata(&Self::container_name_for(environment))
    }

    pub fn create(
        environment: &Environment,
        driver_config: &DriverConfig,
        overrides: &DriverDockerConfigOverrides,
        apt_mirror: Option<&str>,
        archives: &Archives,
    ) -> anyhow::Result<Self> {
        let base_image = overrides.base_image.clone().unwrap_or_else(|| {
            driver_config
                .docker
                .base_image_for_distro(&environment.distro)
        });
        let uid = unsafe { libc::geteuid() }.to_string();
        let gid = unsafe { libc::getegid() }.to_string();
        let build_root = environment.root_dir.to_string_lossy();
        let setup = apt::Setup {
            distro: &environment.distro,
            mirror: apt_mirror,
            archives,
            services: environment.purpose == EnvironmentPurpose::Test,
        };
        let dockerfile = dockerfile(environment, &base_image, &setup);
        let image_fingerprint = environment_fingerprint(&[
            "docker",
            &dockerfile,
            &setup.fingerprint(),
            &environment.distro.codename,
            &uid,
            &gid,
        ]);
        let mut container_fingerprint_parts =
            vec!["docker-container", &image_fingerprint, build_root.as_ref()];
        if let Some(purpose) = environment.purpose.fingerprint_part() {
            container_fingerprint_parts.push(purpose);
        }
        let desired_fingerprint = environment_fingerprint(&container_fingerprint_parts);
        let container_name = Self::container_name_for(environment);
        let mut driver = Self {
            environment: environment.clone(),
            container_name,
            reused_environment: false,
            image_created: None,
        };
        let environment_matches = container_environment_fingerprint(&driver.container_name)?
            .as_deref()
            == Some(&desired_fingerprint);

        let mut reuse = environment.persistence.reuses() && environment_matches;
        if reuse {
            if !driver.container_is_running()? {
                driver.container_start()?;
            }
            if !driver.mount_is_live()? {
                driver.container_remove_force()?;
                reuse = false;
            }
        }

        if reuse {
            driver.reused_environment = true;
        } else {
            // The container may not exist; removal errors don't matter here.
            let _ = Command::new("docker")
                .args(["rm", "-f", &driver.container_name])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();

            let docker_image_name = format!("debmagic-env-{}", &image_fingerprint[..16]);
            driver.image_created = image_created(&docker_image_name)?;
            if driver.image_created.is_none() {
                build_build_image(environment, &dockerfile, &setup, &docker_image_name)?;
                driver.image_created = image_created(&docker_image_name)?;
            }
            run_checked(
                Command::new("docker")
                    .args([
                        "run",
                        "--detach",
                        "--name",
                        &driver.container_name,
                        "--label",
                        &format!("{ENVIRONMENT_LABEL}={desired_fingerprint}"),
                        "--mount",
                    ])
                    .arg(bind_mount_arg(
                        &environment.root_dir,
                        ENVIRONMENT_DIR_IN_CONTAINER,
                    ))
                    .arg(&docker_image_name),
                "starting docker container",
            )?;
        }

        Ok(driver)
    }

    pub fn from_metadata(
        environment: &Environment,
        _driver_config: &DriverConfig,
        metadata: &EnvironmentMetadata,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            environment: environment.clone(),
            container_name: container_name_from_metadata(metadata),
            reused_environment: true,
            image_created: None,
        })
    }

    fn translate_path_in_container(
        &self,
        path_in_source: &Path,
    ) -> Result<PathBuf, std::io::Error> {
        translate_path_in_container(&self.environment.root_dir, path_in_source)
    }

    fn exec(
        &self,
        cmd: &[&str],
        cwd: &Path,
        options: CommandOptions<'_>,
    ) -> std::io::Result<CommandResult> {
        let mut exec_cmd = Command::new("docker");
        exec_cmd.args(["exec", "--workdir"]).arg(cwd);
        if options.terminal {
            exec_cmd.arg("--interactive");
            if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
                exec_cmd.arg("--tty");
            }
        } else {
            println!("[{}] $ {}", self.container_name, cmd.join(" "));
        }
        if options.requires_root || options.user.is_some() {
            exec_cmd.args(["--user", options.user.unwrap_or("root")]);
        }
        for (key, value) in options.env_add {
            exec_cmd.args(["--env", &format!("{key}={value}")]);
        }
        exec_cmd.arg(&self.container_name).args(cmd);
        subprocess::command(exec_cmd).capture(options.capture).run()
    }

    fn with_running_container<T>(
        &self,
        action: impl FnOnce(&Self) -> std::io::Result<T>,
    ) -> std::io::Result<T> {
        let was_running = self.container_is_running().map_err(std::io::Error::other)?;
        if !was_running {
            self.container_start().map_err(std::io::Error::other)?;
        }
        let result = action(self);
        if !was_running {
            self.container_stop().map_err(std::io::Error::other)?;
        }
        result
    }

    fn account(&self, user: &str) -> std::io::Result<crate::driver::PasswdEntry> {
        self.with_running_container(|driver| {
            let mut getent = Command::new("docker");
            getent.args(["exec", &driver.container_name]);
            getent_passwd(getent, user)
        })
    }
}

impl EnvironmentDriver for DriverDocker {
    fn driver_metadata(&self) -> std::collections::HashMap<String, String> {
        container_name_metadata(&self.container_name)
    }

    fn run_command(
        &self,
        cmd: &[&str],
        cwd: &Path,
        options: CommandOptions<'_>,
    ) -> std::io::Result<CommandResult> {
        let container_path = self
            .translate_path_in_container(cwd)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
        if !options.terminal {
            return self.exec(cmd, &container_path, options);
        }
        self.with_running_container(|driver| driver.exec(cmd, &container_path, options))
    }

    fn reset_root(&self) -> std::io::Result<()> {
        let find_cmd = [
            "find",
            ENVIRONMENT_DIR_IN_CONTAINER,
            "-mindepth",
            "1",
            "-delete",
        ];
        println!("[{}] $ {}", self.container_name, find_cmd.join(" "));
        let status = Command::new("docker")
            .args(["exec", "--user", "root", &self.container_name])
            .args(find_cmd)
            .status()?;
        if !status.success() {
            return Err(std::io::Error::other(
                "failed to reset Docker build directory",
            ));
        }
        Ok(())
    }

    fn reused_environment(&self) -> bool {
        self.reused_environment
    }

    fn packages_refreshed_at(&self) -> Option<SystemTime> {
        self.image_created
    }

    fn interactive_shell(&self, cwd: &Path, user: Option<&str>) -> std::io::Result<i32> {
        let account = self.account(user.unwrap_or("root"))?;
        self.run_command(
            &[&account.shell],
            cwd,
            CommandOptions {
                user,
                env_add: &[("HOME", &account.home)],
                terminal: true,
                ..Default::default()
            },
        )
        .map(|result| result.exit_code)
    }

    fn driver_type(&self) -> DriverType {
        DriverType::Docker
    }

    fn isolation_capability(&self) -> IsolationCapability {
        IsolationCapability::Container
    }

    fn probe_resource(&self) -> ResourceStatus {
        match Command::new("docker")
            .args(["inspect", &self.container_name])
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output()
        {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                ResourceStatus::Unreachable
            }
            Err(_) => ResourceStatus::Unreachable,
            Ok(output) if output.status.success() => ResourceStatus::Present,
            Ok(output) => {
                // Docker says "No such object", Podman's docker shim "no such object".
                let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
                if stderr.contains("no such object") || stderr.contains("no such container") {
                    ResourceStatus::Absent
                } else {
                    ResourceStatus::Unreachable
                }
            }
        }
    }

    fn destroy_resource(&self) -> anyhow::Result<()> {
        match self.probe_resource() {
            ResourceStatus::Absent => Ok(()),
            ResourceStatus::Present | ResourceStatus::Unreachable => self.container_remove_force(),
        }
    }
}
