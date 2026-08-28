use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use anyhow::Context as _;
use debmagic_common::distro::{Distro, DistroVersion};
use serde::{Deserialize, Serialize};

use crate::driver::{
    APT_MIRROR_SCRIPT, ContainerSignPrep, DriverType, ENVIRONMENT_DIR_IN_CONTAINER, Environment,
    EnvironmentDriver, EnvironmentMetadata, IsolationCapability, SignLocation, SignRequest,
    config::DriverConfig, container_name_from_metadata, container_name_metadata,
    environment_fingerprint, prepare_container_sign, resource_name, run_checked,
    translate_path_in_container,
};

// The binary name differs between LXD and Incus, but everything else is shared.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum LxdVariant {
    Lxd,
    Incus,
}

impl LxdVariant {
    pub fn binary(self) -> &'static str {
        match self {
            LxdVariant::Lxd => "lxc",
            LxdVariant::Incus => "incus",
        }
    }
}

// The host user is mapped to BUILD_USER_UID/GID via raw.idmap so that the
// bind-mounted source tree is writable from within the container.
const BUILD_USER_UID: u32 = 1000;
const BUILD_USER_GID: u32 = 1000;
const ENVIRONMENT_CONFIG_KEY: &str = "user.debmagic.environment";
const ENVIRONMENT_SETUP_VERSION: &str = "dpkg-dev-norec python3; build-user-v1; raw.idmap-v1";

// ── Config ────────────────────────────────────────────────────────────────────

/// Persistent configuration for both LXD and Incus drivers (from `debmagic.toml`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DriverLxdConfig {
    /// LXD/Incus project to use. `None` means use the default project.
    pub project: Option<String>,
    /// Override base image per distro, keyed by `"<distro>:<codename>"`.
    /// If absent, falls back to `"images:<distro>/<codename>"`.
    pub base_images: std::collections::HashMap<String, String>,
}

impl DriverLxdConfig {
    pub fn base_image_for_distro(&self, variant: LxdVariant, distro: &DistroVersion) -> String {
        self.base_images
            .get(&format!("{}:{}", distro.distro, distro.codename))
            .cloned()
            .unwrap_or_else(|| default_base_image(variant, distro))
    }
}

fn default_base_image(variant: LxdVariant, distro: &DistroVersion) -> String {
    match (&distro.distro, variant, distro.is_devel) {
        // LXD ships a dedicated ubuntu: remote; daily builds are on ubuntu-daily:.
        (Distro::Ubuntu, LxdVariant::Lxd, false) => format!("ubuntu:{}", distro.version),
        (Distro::Ubuntu, LxdVariant::Lxd, true) => format!("ubuntu-daily:{}", distro.version),
        // Incus uses the images: remote for everything; daily via /daily variant.
        (Distro::Ubuntu, LxdVariant::Incus, false) => format!("images:ubuntu/{}", distro.version),
        (Distro::Ubuntu, LxdVariant::Incus, true) => {
            format!("images:ubuntu/{}/daily", distro.codename)
        }
        // Debian images live on images: for both variants, released and devel alike.
        (Distro::Debian, _, _) => {
            format!("images:debian/{}", debian_image_codename(&distro.codename))
        }
        // Custom families must be declared in base_images; this is only a last-resort fallback.
        (Distro::Custom(family), _, _) => format!("images:{family}/{}", distro.codename),
    }
}

// The images remote uses real codenames; changelog aliases like "unstable"
// have no dedicated image and map to sid.
fn debian_image_codename(codename: &str) -> &str {
    match codename {
        "unstable" => "sid",
        other => other,
    }
}

/// Per-invocation overrides (CLI flags).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriverLxdConfigOverrides {
    pub base_image: Option<String>,
    pub project: Option<String>,
}

// ── Shared implementation ─────────────────────────────────────────────────────

pub struct DriverLxd {
    variant: LxdVariant,
    environment: Environment,
    container_name: String,
    /// Resolved project name (None → omit `--project` flag).
    project: Option<String>,
    /// Base image of the distro, used to spin up minimal one-shot containers
    /// (e.g. for signing) that don't need the build environment's tooling.
    base_image: String,
    reused_environment: bool,
}

impl DriverLxd {
    fn project_args(&self) -> Vec<String> {
        match &self.project {
            Some(p) => vec!["--project".to_string(), p.clone()],
            None => vec![],
        }
    }

    fn lxd_cmd(&self, subcommand: &str) -> Command {
        let mut cmd = Command::new(self.variant.binary());
        cmd.args(self.project_args());
        cmd.arg(subcommand);
        cmd
    }

    /// Query the LXD/Incus `list` entry for this container, if it exists.
    fn container_list_entry(&self) -> anyhow::Result<Option<serde_json::Value>> {
        let mut list_cmd = self.lxd_cmd("list");
        list_cmd.args(["--format", "json"]);
        list_cmd.stdout(Stdio::piped());

        let output = list_cmd
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to list containers: {e}"))?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "failed to query {} containers",
                self.variant.binary()
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let containers: Vec<serde_json::Value> = serde_json::from_str(&stdout)
            .map_err(|e| anyhow::anyhow!("Failed to parse container list JSON: {e}"))?;

        Ok(containers.into_iter().find(|container| {
            container.get("name").and_then(|v| v.as_str()) == Some(self.container_name.as_str())
        }))
    }

    fn is_entry_running(entry: &serde_json::Value) -> bool {
        entry
            .get("status")
            .and_then(|v| v.as_str())
            .map(|s| s == "Running")
            .unwrap_or(false)
    }

    fn container_start(&self) -> anyhow::Result<()> {
        run_checked(
            self.lxd_cmd("start").arg(&self.container_name),
            &format!("starting {} container", self.variant.binary()),
        )
    }

    fn container_stop(&self) -> anyhow::Result<()> {
        run_checked(
            self.lxd_cmd("stop").arg(&self.container_name),
            &format!("stopping {} container", self.variant.binary()),
        )
    }

    fn container_delete_force(&self) -> anyhow::Result<()> {
        run_checked(
            self.lxd_cmd("delete")
                .args(["--force", &self.container_name]),
            &format!("removing {} container", self.variant.binary()),
        )
    }

    fn container_environment_fingerprint(&self) -> anyhow::Result<Option<String>> {
        let output = self
            .lxd_cmd("config")
            .args(["get", &self.container_name, ENVIRONMENT_CONFIG_KEY])
            .output()
            .map_err(|error| {
                anyhow::anyhow!(
                    "failed to inspect {} container: {error}",
                    self.variant.binary()
                )
            })?;
        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "failed to inspect {} container {}",
                self.variant.binary(),
                self.container_name
            ));
        }
        let fingerprint = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok((!fingerprint.is_empty()).then_some(fingerprint))
    }

    pub fn create(
        variant: LxdVariant,
        environment: &Environment,
        driver_config: &DriverConfig,
        overrides: &DriverLxdConfigOverrides,
        apt_mirror: Option<&str>,
        proposed: bool,
    ) -> anyhow::Result<Self> {
        let container_name = resource_name(
            "debmagic",
            &environment.package_name,
            &environment.identifier(),
        );

        // Project: CLI override > config file value
        let project = overrides
            .project
            .clone()
            .or_else(|| driver_config.lxd.project.clone());
        let base_image = overrides.base_image.clone().unwrap_or_else(|| {
            driver_config
                .lxd
                .base_image_for_distro(variant, &environment.distro)
        });
        let host_uid = unsafe { libc::geteuid() }.to_string();
        let host_gid = unsafe { libc::getegid() }.to_string();
        let build_root = environment.root_dir.to_string_lossy();
        let proposed_fingerprint = proposed.to_string();
        let mut fingerprint_parts = vec![
            variant.binary(),
            ENVIRONMENT_SETUP_VERSION,
            &base_image,
            &environment.distro.codename,
            apt_mirror.unwrap_or(""),
            &proposed_fingerprint,
            &host_uid,
            &host_gid,
            build_root.as_ref(),
        ];
        if let Some(purpose) = environment.purpose.fingerprint_part() {
            fingerprint_parts.push(purpose);
        }
        let desired_fingerprint = environment_fingerprint(&fingerprint_parts);

        let mut base = Self {
            variant,
            environment: environment.clone(),
            container_name: container_name.clone(),
            project,
            base_image: base_image.clone(),
            reused_environment: false,
        };

        let container_entry = base.container_list_entry()?;
        let environment_matches = container_entry.is_some()
            && base.container_environment_fingerprint()?.as_deref() == Some(&desired_fingerprint);
        let reusing_container = environment.persistent && environment_matches;
        base.reused_environment = reusing_container;

        let mut initialized_container = false;
        let setup_result = (|| -> anyhow::Result<()> {
            if reusing_container {
                let already_running = container_entry
                    .as_ref()
                    .is_some_and(DriverLxd::is_entry_running);
                if !already_running {
                    base.container_start()?;
                }
            } else {
                if container_entry.is_some() {
                    base.container_delete_force()?;
                }

                let mut init = base.lxd_cmd("init");
                if !environment.persistent {
                    init.arg("--ephemeral");
                }
                init.args([&base_image, &container_name]);
                run_checked(
                    &mut init,
                    &format!("initialising {} container", variant.binary()),
                )?;
                initialized_container = true;

                // Map the host user's uid/gid to BUILD_USER_UID inside the container
                // so that files in the bind-mounted build root are writable.
                let host_uid = unsafe { libc::geteuid() };
                let host_gid = unsafe { libc::getegid() };
                if host_uid != 0 {
                    let idmap = format!(
                        "uid {} {}\ngid {} {}",
                        host_uid, BUILD_USER_UID, host_gid, BUILD_USER_GID
                    );
                    run_checked(
                        base.lxd_cmd("config")
                            .arg("set")
                            .arg(&container_name)
                            .arg("raw.idmap")
                            .arg(&idmap),
                        "setting raw.idmap on container",
                    )?;
                }

                let device_name = resource_name(
                    "debmagic-src",
                    &environment.package_name,
                    &environment.identifier(),
                );
                run_checked(
                    base.lxd_cmd("config")
                        .arg("device")
                        .arg("add")
                        .arg(&container_name)
                        .arg(&device_name)
                        .arg("disk")
                        .arg(format!("source={}", environment.root_dir.display()))
                        .arg(format!("path={}", ENVIRONMENT_DIR_IN_CONTAINER)),
                    &format!("mounting build root into {} container", variant.binary()),
                )?;

                run_checked(
                    base.lxd_cmd("start").arg(&container_name),
                    &format!("starting {} container", variant.binary()),
                )?;

                if matches!(environment.distro.distro, Distro::Ubuntu) {
                    base.exec_in_container(&["cloud-init", "status", "--wait"], None, true, &[])
                        .map_err(|e| {
                            anyhow::anyhow!("Error waiting for cloud-init to finish: {e}")
                        })?;
                }
            }

            // Re-run on every reuse of a persistent container too, so that a
            // previous invocation that crashed before finishing this setup (or a
            // long-lived incremental container with an aging package cache)
            // doesn't leave `apt-get build-dep` unable to resolve anything.
            base.exec_in_container_checked(&["apt-get", "update"], None, true, &[])
                .map_err(|e| anyhow::anyhow!("Error running apt-get update in container: {e}"))?;

            if !reusing_container {
                // Install the base tooling that stock images don't include.
                // build-essential is intentionally omitted: binary builds pull
                // it in explicitly, so source-only environments stay lean.
                // build-dep is omitted too: build.rs runs it for every driver
                // against the real mounted source tree.
                base.exec_in_container_checked(
                    &[
                        "apt-get",
                        "install",
                        "-y",
                        "--no-install-recommends",
                        "dpkg-dev",
                        "python3",
                    ],
                    None,
                    true,
                    &[],
                )
                .map_err(|e| anyhow::anyhow!("Error installing base packages in container: {e}"))?;

                let ensure_build_user = format!(
                    "getent group {gid} >/dev/null || groupadd --gid {gid} debmagic; \
                     getent passwd {uid} >/dev/null || useradd --uid {uid} --gid {gid} -m debmagic",
                    uid = BUILD_USER_UID,
                    gid = BUILD_USER_GID,
                );
                base.exec_in_container_checked(&["sh", "-ec", &ensure_build_user], None, true, &[])
                    .map_err(|e| anyhow::anyhow!("Error creating build user in container: {e}"))?;

                if apt_mirror.is_some() || proposed {
                    let script_path = environment.temp_dir().join("mirror.py");
                    fs::write(&script_path, APT_MIRROR_SCRIPT)?;
                    let container_script_path = base.translate_path_in_container(&script_path)?;
                    let mut args = vec![
                        "python3".to_string(),
                        container_script_path.to_string_lossy().into_owned(),
                        "--codename".to_string(),
                        environment.distro.codename.clone(),
                    ];
                    if let Some(mirror) = apt_mirror {
                        args.extend(["--mirror".to_string(), mirror.to_string()]);
                    }
                    if proposed {
                        args.push("--proposed".to_string());
                    }
                    let args = args.iter().map(String::as_str).collect::<Vec<_>>();
                    base.exec_in_container_checked(&args, None, true, &[])
                        .map_err(|e| anyhow::anyhow!("Error configuring apt sources: {e}"))?;
                    base.exec_in_container_checked(&["apt-get", "update"], None, true, &[])
                        .map_err(|e| {
                            anyhow::anyhow!("Error updating configured apt sources: {e}")
                        })?;
                }

                run_checked(
                    base.lxd_cmd("config")
                        .arg("set")
                        .arg(&container_name)
                        .arg(ENVIRONMENT_CONFIG_KEY)
                        .arg(&desired_fingerprint),
                    &format!("recording {} environment", variant.binary()),
                )?;
            }
            Ok(())
        })();

        if let Err(error) = setup_result {
            if initialized_container && let Err(cleanup_error) = base.container_delete_force() {
                return Err(error.context(format!(
                    "also failed to remove {} container: {cleanup_error}",
                    variant.binary()
                )));
            }
            return Err(error);
        }

        Ok(base)
    }

    pub fn from_metadata(
        variant: LxdVariant,
        environment: &Environment,
        driver_config: &DriverConfig,
        metadata: &EnvironmentMetadata,
    ) -> anyhow::Result<Self> {
        let project = metadata.driver_metadata.get("project").cloned();

        Ok(Self {
            variant,
            environment: environment.clone(),
            container_name: container_name_from_metadata(metadata)?,
            project,
            base_image: driver_config
                .lxd
                .base_image_for_distro(variant, &environment.distro),
            reused_environment: true,
        })
    }

    fn translate_path_in_container(
        &self,
        path_in_source: &Path,
    ) -> Result<PathBuf, std::io::Error> {
        translate_path_in_container(&self.environment.root_dir, path_in_source)
    }

    /// Run `action` with the container running, restoring a previously
    /// stopped container to the stopped state afterwards.
    fn with_running_container(
        &self,
        action: impl FnOnce(&Self) -> std::io::Result<()>,
    ) -> std::io::Result<()> {
        let entry = self
            .container_list_entry()
            .map_err(std::io::Error::other)?
            .ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::NotFound, "container not found")
            })?;
        let was_running = Self::is_entry_running(&entry);
        if !was_running {
            self.container_start().map_err(std::io::Error::other)?;
        }
        let result = action(self);
        let stop_result = if was_running {
            Ok(())
        } else {
            self.container_stop()
        };
        match (result, stop_result) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(e), Ok(())) => Err(e),
            (Ok(()), Err(e)) => Err(std::io::Error::other(e)),
            (Err(e), Err(stop_error)) => Err(std::io::Error::other(format!(
                "{e}; also failed to stop {} container: {stop_error}",
                self.variant.binary()
            ))),
        }
    }

    fn exec_in_container(
        &self,
        cmd: &[&str],
        workdir: Option<&Path>,
        as_root: bool,
        env_add: &[(&str, &str)],
    ) -> std::io::Result<i32> {
        println!("[{}] $ {}", self.container_name, cmd.join(" "));

        let mut exec_cmd = self.lxd_cmd("exec");
        exec_cmd.arg(&self.container_name);

        if let Some(wd) = workdir {
            exec_cmd.args(["--cwd", &wd.to_string_lossy()]);
        }

        if !as_root {
            // Run as BUILD_USER_UID — the host user is mapped to this uid via
            // raw.idmap, so it owns the bind-mounted source tree inside.
            exec_cmd.args(["--user", &BUILD_USER_UID.to_string()]);
            exec_cmd.args(["--group", &BUILD_USER_GID.to_string()]);
        }

        for (key, value) in env_add {
            exec_cmd.args(["--env", &format!("{key}={value}")]);
        }

        exec_cmd.arg("--");
        exec_cmd.args(cmd);

        let status = exec_cmd.status()?;
        Ok(status.code().unwrap_or(-1))
    }

    fn exec_in_container_checked(
        &self,
        cmd: &[&str],
        workdir: Option<&Path>,
        as_root: bool,
        env_add: &[(&str, &str)],
    ) -> std::io::Result<()> {
        let code = self.exec_in_container(cmd, workdir, as_root, env_add)?;
        if code != 0 {
            return Err(std::io::Error::other(format!(
                "{} exec failed with exit code {code}",
                self.variant.binary()
            )));
        }
        Ok(())
    }
}

impl EnvironmentDriver for DriverLxd {
    fn driver_metadata(&self) -> std::collections::HashMap<String, String> {
        let mut meta = container_name_metadata(&self.container_name);
        if let Some(ref p) = self.project {
            meta.insert("project".to_string(), p.clone());
        }
        meta
    }

    fn run_command(
        &self,
        cmd: &[&str],
        cwd: &Path,
        requires_root: bool,
        env_add: &[(&str, &str)],
    ) -> std::io::Result<i32> {
        let container_path = self
            .translate_path_in_container(cwd)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

        self.exec_in_container(cmd, Some(&container_path), requires_root, env_add)
    }

    fn cleanup(&self) -> anyhow::Result<()> {
        if self.environment.persistent {
            Ok(())
        } else {
            // Ephemeral containers are auto-deleted after stopping.
            self.container_stop()
        }
    }

    fn reset_root(&self) -> std::io::Result<()> {
        self.with_running_container(|driver| {
            driver.exec_in_container_checked(
                &[
                    "find",
                    ENVIRONMENT_DIR_IN_CONTAINER,
                    "-mindepth",
                    "1",
                    "-delete",
                ],
                None,
                true,
                &[],
            )
        })
    }

    fn reused_environment(&self) -> bool {
        self.reused_environment
    }

    fn interactive_shell(&self, cwd: &Path) -> std::io::Result<()> {
        let workdir = self.translate_path_in_container(cwd)?;
        self.with_running_container(|driver| {
            let mut command = driver.lxd_cmd("exec");
            command
                .arg(&driver.container_name)
                .args(["--cwd", &workdir.to_string_lossy()]);
            let status = command.arg("--").args(["/usr/bin/env", "bash"]).status()?;
            if !status.success() {
                return Err(std::io::Error::other(format!(
                    "{} shell failed",
                    driver.variant.binary()
                )));
            }
            Ok(())
        })
    }

    fn driver_type(&self) -> DriverType {
        match self.variant {
            LxdVariant::Lxd => DriverType::Lxd,
            LxdVariant::Incus => DriverType::Incus,
        }
    }

    fn isolation_capability(&self) -> IsolationCapability {
        IsolationCapability::Container
    }

    fn sign_changes(&self, request: &SignRequest) -> anyhow::Result<()> {
        let Some(prep) = prepare_container_sign(request, &self.environment.temp_dir())? else {
            return Ok(());
        };

        match request.location {
            SignLocation::BuildContainer => self.sign_in_build_container(request, &prep),
            SignLocation::EphemeralContainer => self.sign_in_ephemeral_container(request, &prep),
            SignLocation::Host => unreachable!("handled above"),
        }
    }
}

impl DriverLxd {
    /// Sign inside the running build container: add the staging dir and a
    /// proxy for the agent socket as devices, then run `debsign` in the work
    /// dir where `dpkg-buildpackage` left the `.changes`.
    fn sign_in_build_container(
        &self,
        request: &SignRequest,
        prep: &ContainerSignPrep,
    ) -> anyhow::Result<()> {
        use crate::signing;

        self.with_running_container(|_| Ok(()))
            .map_err(|e| anyhow::anyhow!("build container is not available for signing: {e}"))?;

        let bin = self.variant.binary();
        let container = &self.container_name;
        // Devices on a running container are hot-plugged; remove them again
        // after signing so a persistent container doesn't keep them around.
        let staging_device = resource_name(
            "debmagic-sign",
            &self.environment.package_name,
            &self.environment.identifier(),
        );
        let socket_device = format!("{staging_device}-agent");
        let socket_dir = Path::new("/tmp/debmagic-gpg");
        let socket = socket_dir.join(signing::GPG_SOCKET_FILENAME);

        let cleanup = |driver: &Self| {
            for dev in [&staging_device, &socket_device] {
                let _ = driver
                    .lxd_cmd("config")
                    .args(["device", "remove"])
                    .arg(container)
                    .arg(dev)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            }
        };

        let result = (|| -> anyhow::Result<()> {
            // The proxy device hot-plugs its listen socket, so its parent dir
            // must already exist inside the running container.
            self.exec_in_container_checked(
                &["mkdir", "-p", &socket_dir.to_string_lossy()],
                None,
                true,
                &[],
            )
            .map_err(|e| anyhow::anyhow!("failed to create gpg socket dir in container: {e}"))?;

            run_checked(
                self.lxd_cmd("config")
                    .arg("device")
                    .arg("add")
                    .arg(container)
                    .arg(&staging_device)
                    .arg("disk")
                    .arg(format!("source={}", prep.staging_dir.display()))
                    .arg(format!("path={}", signing::SIGN_STAGING_IN_CONTAINER))
                    .arg("readonly=true"),
                "mounting signing material into build container",
            )?;
            // Forward the host agent socket via a proxy listening in a
            // container-local /tmp dir (the build root is idmapped, which the
            // proxy can't write to as root).
            let socket_owner = std::fs::metadata(&prep.gpg.agent_extra_socket)
                .context("failed to stat host gpg-agent socket")?;
            run_checked(
                self.lxd_cmd("config")
                    .arg("device")
                    .arg("add")
                    .arg(container)
                    .arg(&socket_device)
                    .arg("proxy")
                    .arg("bind=container")
                    .arg(format!(
                        "connect=unix:{}",
                        prep.gpg.agent_extra_socket.display()
                    ))
                    .arg(format!("listen=unix:{}", socket.display()))
                    .arg("uid=0")
                    .arg("gid=0")
                    .arg(format!("security.uid={}", socket_owner.uid()))
                    .arg(format!("security.gid={}", socket_owner.gid())),
                "forwarding gpg-agent socket into build container",
            )?;

            let work_dir = Path::new(ENVIRONMENT_DIR_IN_CONTAINER).join("work");
            let scripts = signing::sign_container_scripts(
                signing::ContainerSignMode::Build {
                    work_dir: &work_dir,
                    staging_dir: Path::new(signing::SIGN_STAGING_IN_CONTAINER),
                },
                &socket,
                &prep.changes,
                &prep.gpg.sign_key,
            );
            let work = self.environment.work_dir();
            self.exec_in_container_checked(&["sh", "-ec", &scripts.setup], Some(&work), true, &[])
                .map_err(|e| {
                    anyhow::anyhow!("preparing the {bin} build container for signing: {e}")
                })?;
            request.notify_signing();
            self.exec_in_container_checked(&["sh", "-ec", &scripts.sign], Some(&work), true, &[])
                .map_err(|e| anyhow::anyhow!("signing in the {bin} build container failed: {e}"))?;
            Ok(())
        })();

        cleanup(self);
        result
    }

    /// Sign in a minimal, throwaway container with the agent socket forwarded
    /// and the output dir mounted.
    fn sign_in_ephemeral_container(
        &self,
        request: &SignRequest,
        prep: &ContainerSignPrep,
    ) -> anyhow::Result<()> {
        use crate::signing;

        let output_dir = request
            .changes_file
            .parent()
            .context("changes file has no parent directory")?;
        let gpg_dir = self.environment.temp_dir().join("sign-gpg");
        std::fs::create_dir_all(&gpg_dir).context("failed to create gpg socket dir")?;
        // The lxd proxy runs on the host as real root, which an idmapped
        // mount (raw.idmap) maps to "other" on a dir owned by the host user —
        // so the proxy needs o+wx to create its listen socket here. The dir
        // is throwaway (build temp dir) and holds only the transient socket.
        std::fs::set_permissions(&gpg_dir, std::fs::Permissions::from_mode(0o777))
            .context("failed to make gpg socket dir writable for the proxy")?;
        // No chown needed: raw.idmap maps container root to the host user.
        let agent_socket =
            Path::new(signing::GPG_DIR_IN_CONTAINER).join(signing::GPG_SOCKET_FILENAME);
        let scripts = signing::sign_container_scripts(
            signing::ContainerSignMode::Ephemeral { chown_to: None },
            &agent_socket,
            &prep.changes,
            &prep.gpg.sign_key,
        );
        let gpg = &prep.gpg;

        let sign_container = resource_name(
            "debmagic-sign",
            &self.environment.package_name,
            &self.environment.identifier(),
        );
        let bin = self.variant.binary();
        // The sign container is ephemeral; it disappears when stopped.
        let mut init = self.lxd_cmd("init");
        init.arg("--ephemeral");
        init.args([&self.base_image, sign_container.as_str()]);
        run_checked(&mut init, &format!("initialising {bin} sign container"))?;

        let result = (|| -> anyhow::Result<()> {
            let host_uid = unsafe { libc::geteuid() };
            let host_gid = unsafe { libc::getegid() };
            if host_uid != 0 {
                let idmap = format!("uid {host_uid} 0\ngid {host_gid} 0");
                run_checked(
                    self.lxd_cmd("config")
                        .arg("set")
                        .arg(&sign_container)
                        .arg("raw.idmap")
                        .arg(&idmap),
                    "setting raw.idmap on sign container",
                )?;
            }

            run_checked(
                self.lxd_cmd("config")
                    .arg("device")
                    .arg("add")
                    .arg(&sign_container)
                    .arg("debmagic-output")
                    .arg("disk")
                    .arg(format!("source={}", output_dir.display()))
                    .arg(format!("path={}", signing::OUTPUT_DIR_IN_CONTAINER)),
                "mounting output directory into sign container",
            )?;
            run_checked(
                self.lxd_cmd("config")
                    .arg("device")
                    .arg("add")
                    .arg(&sign_container)
                    .arg("debmagic-sign-staging")
                    .arg("disk")
                    .arg(format!("source={}", prep.staging_dir.display()))
                    .arg(format!("path={}", signing::SIGN_STAGING_IN_CONTAINER))
                    .arg("readonly=true"),
                "mounting signing material into sign container",
            )?;
            run_checked(
                self.lxd_cmd("config")
                    .arg("device")
                    .arg("add")
                    .arg(&sign_container)
                    .arg("debmagic-gpg-dir")
                    .arg("disk")
                    .arg(format!("source={}", gpg_dir.display()))
                    .arg(format!("path={}", signing::GPG_DIR_IN_CONTAINER)),
                "mounting gpg socket dir into sign container",
            )?;
            // Forward the host gpg-agent's extra socket via a proxy device,
            // listening inside the mounted output dir: the proxy needs the
            // listen path's parent to be a host-mounted directory it can
            // write to (rootfs dirs don't exist yet when the device is set
            // up, and /run is mounted over at boot). security.uid/gid select
            // the credentials lxd connects to the host socket with; gpg-agent
            // rejects connections that don't come from the socket's owner, so
            // this must be the host user's ids, not the default root.
            let socket_owner = std::fs::metadata(&gpg.agent_extra_socket)
                .context("failed to stat host gpg-agent socket")?;
            run_checked(
                self.lxd_cmd("config")
                    .arg("device")
                    .arg("add")
                    .arg(&sign_container)
                    .arg("debmagic-gpg-agent")
                    .arg("proxy")
                    .arg("bind=container")
                    .arg(format!("connect=unix:{}", gpg.agent_extra_socket.display()))
                    .arg(format!(
                        "listen=unix:{}/{}",
                        signing::GPG_DIR_IN_CONTAINER,
                        signing::GPG_SOCKET_FILENAME
                    ))
                    .arg("uid=0")
                    .arg("gid=0")
                    .arg(format!("security.uid={}", socket_owner.uid()))
                    .arg(format!("security.gid={}", socket_owner.gid())),
                "forwarding gpg-agent socket into sign container",
            )?;

            run_checked(
                self.lxd_cmd("start").arg(&sign_container),
                &format!("starting {bin} sign container"),
            )?;

            // Same first-boot caveat as the build container: on Ubuntu images
            // cloud-init may still hold the apt lock.
            if matches!(self.environment.distro.distro, Distro::Ubuntu) {
                self.exec_in_container_checked(
                    &["cloud-init", "status", "--wait"],
                    None,
                    true,
                    &[],
                )
                .map_err(|e| anyhow::anyhow!("Error waiting for cloud-init to finish: {e}"))?;
            }

            println!(
                "[{bin}] $ signing {} in a minimal {} container",
                request.changes_file.display(),
                self.environment.distro.codename
            );

            let mut setup = self.lxd_cmd("exec");
            setup.arg(&sign_container);
            setup.arg("--");
            setup.args(["sh", "-ec", &scripts.setup]);
            run_checked(&mut setup, "preparing the sign container")?;

            // Notify right before debsign triggers the gpg touch prompt; the
            // setup above can take long enough to miss it otherwise.
            request.notify_signing();
            let mut sign = self.lxd_cmd("exec");
            sign.arg(&sign_container);
            sign.arg("--");
            sign.args(["sh", "-ec", &scripts.sign]);
            run_checked(&mut sign, "signing in container")?;
            Ok(())
        })();

        let mut stop = self.lxd_cmd("stop");
        let stop_result = run_checked(
            stop.arg(&sign_container),
            &format!("stopping {bin} sign container"),
        );
        result.and(stop_result)
    }
}
