use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use anyhow::{Context, anyhow};
use debmagic_common::distro::DistroVersion;
use serde::{Deserialize, Serialize};

use crate::build::{
    common::{
        APT_MIRROR_SCRIPT, BUILD_DIR_IN_CONTAINER, BuildConfig, BuildDriver, BuildDriverType,
        BuildMetadata, DriverSpecificBuildMetadata, container_name_from_metadata,
        container_name_metadata, environment_fingerprint, resource_name, run_checked,
        translate_path_in_container,
    },
    config::DriverConfig,
};

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
RUN apt-get update && apt-get install -y dpkg-dev python3
{apt_mirror_setup}
RUN set -e; \
    getent group "$USER_GID" >/dev/null || groupadd --gid "$USER_GID" debmagic; \
    getent passwd "$USER_UID" >/dev/null || useradd --uid "$USER_UID" --gid "$USER_GID" -m debmagic
RUN mkdir -p {build_dir} && chown $USER_UID:$USER_GID {build_dir}
USER $USER_UID:$USER_GID
ENTRYPOINT ["sleep", "infinity"]
"#;

// Ubuntu images ship python3 already; installed explicitly above for the
// others (e.g. Debian's slim images) before the mirror script needs it.
const APT_MIRROR_SCRIPT_FILENAME: &str = "mirror.py";

/// Quote `s` as a single POSIX shell word for embedding in a Dockerfile
/// `RUN` instruction (which is interpreted by `/bin/sh -c`).
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

pub struct DriverDocker {
    config: BuildConfig,
    container_name: String,
    /// Base image of the distro, used to spin up minimal one-shot containers
    /// (e.g. for signing) that don't need the build environment's tooling.
    base_image: String,
    reused_environment: bool,
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
    config: &BuildConfig,
    base_image: &str,
    apt_mirror: Option<&str>,
    proposed: bool,
    image_name: &str,
) -> anyhow::Result<()> {
    let apt_mirror_setup = if apt_mirror.is_some() || proposed {
        fs::write(
            config.build_temp_dir().join(APT_MIRROR_SCRIPT_FILENAME),
            APT_MIRROR_SCRIPT,
        )
        .map_err(|e| anyhow!("Failed to write apt mirror script: {e}"))?;

        let mut args = vec![
            "--codename".to_string(),
            shell_quote(&config.distro.codename),
        ];
        if let Some(mirror) = apt_mirror {
            args.extend(["--mirror".to_string(), shell_quote(mirror)]);
        }
        if proposed {
            args.push("--proposed".to_string());
        }

        let setup = format!(
            "COPY {script} /tmp/{script}\nRUN python3 /tmp/{script} {args} && rm /tmp/{script} && apt-get update\n",
            script = APT_MIRROR_SCRIPT_FILENAME,
            args = args.join(" "),
        );
        setup
    } else {
        String::new()
    };

    let formatted_dockerfile = DOCKERFILE_TEMPLATE
        .replace("{base_image}", base_image)
        .replace("{apt_mirror_setup}", &apt_mirror_setup)
        .replace("{build_dir}", BUILD_DIR_IN_CONTAINER);

    let dockerfile_path = config.build_temp_dir().join("Dockerfile");
    fs::write(&dockerfile_path, formatted_dockerfile)
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
        .arg(config.build_temp_dir());

    run_checked(&mut build_cmd, "building docker image")?;

    Ok(())
}

fn does_image_exist(image_name: &str) -> anyhow::Result<bool> {
    let status = Command::new("docker")
        .args(["image", "inspect", image_name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| anyhow!("Failed to inspect Docker image: {error}"))?;
    Ok(status.success())
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

    pub fn create(
        config: &BuildConfig,
        driver_config: &DriverConfig,
        overrides: &DriverDockerConfigOverrides,
        apt_mirror: Option<&str>,
        proposed: bool,
    ) -> anyhow::Result<Self> {
        let base_image = overrides
            .base_image
            .clone()
            .unwrap_or_else(|| driver_config.docker.base_image_for_distro(&config.distro));
        let uid = unsafe { libc::geteuid() }.to_string();
        let gid = unsafe { libc::getegid() }.to_string();
        let build_root = config.build_root_dir.to_string_lossy();
        let proposed_fingerprint = proposed.to_string();
        let image_fingerprint = environment_fingerprint(&[
            "docker",
            DOCKERFILE_TEMPLATE,
            APT_MIRROR_SCRIPT,
            &base_image,
            &config.distro.codename,
            apt_mirror.unwrap_or(""),
            &proposed_fingerprint,
            &uid,
            &gid,
        ]);
        let desired_fingerprint =
            environment_fingerprint(&["docker-container", &image_fingerprint, build_root.as_ref()]);
        let container_name = resource_name(
            "debmagic",
            &config.package_name,
            &sanitize_docker_reference(&config.build_identifier()),
        );
        let mut driver = Self {
            config: config.clone(),
            container_name,
            base_image: base_image.clone(),
            reused_environment: false,
        };
        let environment_matches = container_environment_fingerprint(&driver.container_name)?
            .as_deref()
            == Some(&desired_fingerprint);
        driver.reused_environment = config.persistent && environment_matches;
        let created_container;

        if config.persistent && environment_matches {
            created_container = false;
            if !driver.container_is_running()? {
                driver.container_start()?;
            }
        } else {
            // The container may not exist; removal errors don't matter here.
            let _ = Command::new("docker")
                .args(["rm", "-f", &driver.container_name])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();

            let docker_image_name = format!("debmagic-env-{}", &image_fingerprint[..16]);
            if !does_image_exist(&docker_image_name)? {
                build_build_image(
                    config,
                    &base_image,
                    apt_mirror,
                    proposed,
                    &docker_image_name,
                )?;
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
                        &config.build_root_dir,
                        BUILD_DIR_IN_CONTAINER,
                    ))
                    .arg(&docker_image_name),
                "starting docker container",
            )?;
            created_container = true;
        }

        let update_result = driver
            .run_command(&["apt-get", "update"], &config.build_source_dir(), true)
            .map_err(|error| anyhow!("Error running apt-get update in container: {error}"));
        if let Err(error) = update_result {
            if created_container && let Err(cleanup_error) = driver.container_remove_force() {
                return Err(error.context(format!(
                    "also failed to remove Docker container: {cleanup_error}"
                )));
            }
            return Err(error);
        }

        Ok(driver)
    }

    pub fn from_build_metadata(
        config: &BuildConfig,
        driver_config: &DriverConfig,
        build_metadata: &BuildMetadata,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            config: config.clone(),
            container_name: container_name_from_metadata(build_metadata)?,
            base_image: driver_config.docker.base_image_for_distro(&config.distro),
            reused_environment: true,
        })
    }

    fn translate_path_in_container(
        &self,
        path_in_source: &Path,
    ) -> Result<PathBuf, std::io::Error> {
        translate_path_in_container(&self.config.build_root_dir, path_in_source)
    }
}

impl BuildDriver for DriverDocker {
    fn get_build_metadata(&self) -> DriverSpecificBuildMetadata {
        container_name_metadata(&self.container_name)
    }

    fn run_command_env(
        &self,
        cmd: &[&str],
        cwd: &Path,
        requires_root: bool,
        env_add: &[(&str, &str)],
    ) -> std::io::Result<()> {
        let container_path = self
            .translate_path_in_container(cwd)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

        println!("[{}] $ {}", self.container_name, cmd.join(" "));

        let mut exec_cmd = Command::new("docker");
        exec_cmd.args(["exec", "--workdir"]);
        exec_cmd.arg(container_path);

        if requires_root {
            exec_cmd.args(["--user", "root"]);
        }

        for (key, value) in env_add {
            exec_cmd.args(["--env", &format!("{key}={value}")]);
        }

        exec_cmd.arg(&self.container_name);
        exec_cmd.args(cmd);

        let status = exec_cmd.status()?;
        if !status.success() {
            return Err(std::io::Error::other("Docker exec failed"));
        }
        Ok(())
    }

    fn cleanup(&self) -> anyhow::Result<()> {
        if self.config.persistent {
            Ok(())
        } else {
            self.container_remove_force()
        }
    }

    fn reset_build_root(&self) -> std::io::Result<()> {
        let find_cmd = ["find", BUILD_DIR_IN_CONTAINER, "-mindepth", "1", "-delete"];
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

    fn interactive_shell(&self, cwd: &Path) -> std::io::Result<()> {
        let workdir = self.translate_path_in_container(cwd)?;
        let was_running = self.container_is_running().map_err(std::io::Error::other)?;
        if !was_running {
            self.container_start().map_err(std::io::Error::other)?;
        }

        let mut command = Command::new("docker");
        command
            .args(["exec", "-it", "--user", "root", "--workdir"])
            .arg(&workdir);
        let status = command
            .args([&self.container_name, "/usr/bin/env", "bash"])
            .status();
        if !was_running {
            self.container_stop().map_err(std::io::Error::other)?;
        }
        if !status?.success() {
            return Err(std::io::Error::other("Docker shell failed"));
        }

        Ok(())
    }

    fn driver_type(&self) -> BuildDriverType {
        BuildDriverType::Docker
    }

    fn sign_changes(
        &self,
        changes_file: &Path,
        gpg: Option<&crate::build::signing::GpgForwarding>,
    ) -> anyhow::Result<()> {
        use crate::build::signing;

        let gpg = gpg.context("docker container signing needs gpg forwarding info")?;
        let output_dir = changes_file
            .parent()
            .context("changes file has no parent directory")?;
        let staging_dir = self.config.build_temp_dir().join("sign");
        signing::stage_signing_material(&staging_dir, &gpg.sign_key)?;
        let script = signing::sign_container_script(
            signing::changes_filename(changes_file)?,
            &gpg.sign_key,
            // The sign container's root is not id-mapped; fix ownership of
            // files it rewrites so the host user can manage them afterwards.
            Some((unsafe { libc::geteuid() }, unsafe { libc::getegid() })),
        );

        println!(
            "[docker] $ signing {} in a minimal {} container",
            changes_file.display(),
            self.config.distro.codename
        );
        run_checked(
            Command::new("docker")
                .args(["run", "--rm", "--init"])
                .args([
                    "--mount",
                    &format!(
                        "{},readonly",
                        bind_mount_arg(&staging_dir, signing::SIGN_STAGING_IN_CONTAINER)
                    ),
                ])
                .arg(format!(
                    "--mount=type=bind,src={},dst={},readonly",
                    gpg.agent_extra_socket.display(),
                    signing::GPG_SOCKET_IN_CONTAINER
                ))
                .args([
                    "--mount",
                    &bind_mount_arg(output_dir, signing::OUTPUT_DIR_IN_CONTAINER),
                ])
                .arg(&self.base_image)
                .args(["sh", "-ec", &script]),
            "signing in docker container",
        )?;
        Ok(())
    }
}
