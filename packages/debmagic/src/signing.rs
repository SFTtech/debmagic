//! GPG signing of build artifacts (`.changes`/`.dsc`) via `debsign`.
//!
//! Signing can happen on the host (traditional, requires `devscripts`
//! locally) or inside a minimal same-distro container. In the container case
//! the host's gpg-agent *extra* socket is forwarded in, so private key
//! material never leaves the host — the agent on the host performs the
//! signing operations, and only the public key is imported into the
//! container's keyring.

use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use anyhow::{Context, anyhow};
use serde::{Deserialize, Serialize};

use crate::driver::run_checked;

/// Directory holding the forwarded gpg-agent socket, mounted read-write
/// into sign containers at [`GPG_DIR_IN_CONTAINER`]. The lxd proxy's listen
/// path must live in a host-mounted directory: dirs of the container rootfs
/// don't exist yet when the proxy is set up, /run gets mounted over at boot,
/// and the user's output dir is not ours to litter in.
pub const GPG_DIR_IN_CONTAINER: &str = "/debmagic-gpg";
/// Socket filename created inside the gpg dir.
pub const GPG_SOCKET_FILENAME: &str = "S.gpg-agent";
/// Directory mounted read-only into sign containers, holding the exported
/// public key and ownertrust line produced on the host.
pub const SIGN_STAGING_IN_CONTAINER: &str = "/debmagic-sign";
/// Mount point of the output directory inside sign containers.
pub const OUTPUT_DIR_IN_CONTAINER: &str = "/debmagic-output";

pub const PUBKEY_FILE: &str = "pubkey.asc";
pub const OWNERTRUST_FILE: &str = "ownertrust.txt";

/// Selects where `debsign` runs.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, clap::ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignWith {
    /// Use the host if `debsign` is available there, otherwise a container
    /// (requires a containerized build driver).
    #[default]
    Auto,
    /// Always sign on the host with `debsign`.
    Host,
    /// Sign inside a minimal, separate container of the same distro,
    /// forwarding the host's gpg-agent socket. Requires `sign_key` to be set.
    Separate,
    /// Sign inside the build container itself (no separate container is
    /// started), forwarding the host's gpg-agent socket. Requires `sign_key`
    /// and a containerized build driver.
    Build,
}

/// Everything needed to GPG-sign inside a container: the host's gpg-agent
/// extra socket plus the public key to seed the container's keyring with.
#[derive(Clone)]
pub struct GpgForwarding {
    pub agent_extra_socket: PathBuf,
    pub sign_key: String,
}

/// Resolve the host's gpg-agent *extra* socket — the restricted variant
/// intended for forwarding into chroots/containers (signing works, key
/// export and management don't).
pub fn gpg_agent_extra_socket() -> anyhow::Result<PathBuf> {
    let output = Command::new("gpgconf")
        .args(["--list-dirs", "agent-extra-socket"])
        .stdout(Stdio::piped())
        .output()
        .context("failed to run gpgconf; is gpg installed?")?;
    if !output.status.success() {
        return Err(anyhow!(
            "gpgconf --list-dirs agent-extra-socket failed; is gpg-agent set up?"
        ));
    }
    let path = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim().to_string());
    if !path.exists() {
        return Err(anyhow!(
            "gpg-agent extra socket {} does not exist; is gpg-agent running?",
            path.display()
        ));
    }
    Ok(path)
}

/// Verify that the host gpg setup can sign with `sign_key` (secret key
/// available via the agent). Intended as a pre-flight check so builds don't
/// fail at the signing step after all the work is done.
pub fn check_signing_key_available(sign_key: &str) -> anyhow::Result<()> {
    let output = Command::new("gpg")
        .args(["--batch", "--list-secret-keys", sign_key])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .context("failed to run gpg; is it installed?")?;
    if !output.status.success() || output.stdout.is_empty() {
        return Err(anyhow!(
            "no secret key for '{sign_key}' available to gpg; \
             import it on the host or pick a different sign_key"
        ));
    }
    Ok(())
}

/// Export the public key for `sign_key` from the host keyring.
pub fn export_public_key(sign_key: &str) -> anyhow::Result<Vec<u8>> {
    let output = Command::new("gpg")
        .args(["--batch", "--export", sign_key])
        .stdout(Stdio::piped())
        .output()
        .context("failed to run gpg --export")?;
    if !output.status.success() || output.stdout.is_empty() {
        return Err(anyhow!(
            "failed to export public key for '{sign_key}' from the host keyring"
        ));
    }
    Ok(output.stdout)
}

/// Fingerprint of the key `debsign` will use, for ownertrust seeding.
pub fn key_fingerprint(sign_key: &str) -> anyhow::Result<String> {
    let output = Command::new("gpg")
        .args(["--batch", "--with-colons", "--list-secret-keys", sign_key])
        .stdout(Stdio::piped())
        .output()
        .context("failed to run gpg --list-secret-keys")?;
    if !output.status.success() {
        return Err(anyhow!("failed to look up fingerprint for '{sign_key}'"));
    }
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let fields: Vec<&str> = line.split(':').collect();
        if fields.first() == Some(&"fpr")
            && let Some(fpr) = fields.get(9)
        {
            return Ok(fpr.to_string());
        }
    }
    Err(anyhow!("no fingerprint found for key '{sign_key}'"))
}

/// Stage the files a sign container needs (exported public key + ownertrust)
/// into `staging_dir` on the host; the drivers mount it read-only at
/// [`SIGN_STAGING_IN_CONTAINER`].
pub fn stage_signing_material(staging_dir: &Path, sign_key: &str) -> anyhow::Result<()> {
    std::fs::create_dir_all(staging_dir)?;
    std::fs::write(staging_dir.join(PUBKEY_FILE), export_public_key(sign_key)?)?;
    // Ownertrust format: "<fingerprint>:<trust level>:"; 6 = ultimate. The
    // key is the user's own, freshly imported into a throwaway keyring.
    let ownertrust = format!("{}:6:\n", key_fingerprint(sign_key)?);
    std::fs::write(staging_dir.join(OWNERTRUST_FILE), ownertrust)?;
    Ok(())
}

/// The two phases of in-container signing, split so the host can send the
/// "touch your key" notification after the slow setup but immediately before
/// `debsign` runs.
pub struct SignContainerScripts {
    /// Links the forwarded agent socket, installs debsign, seeds the keyring.
    pub setup: String,
    /// Runs `debsign` (and the ownership fixup). Triggers the gpg touch prompt.
    pub sign: String,
}

/// How the container signs, which sets where the artifacts live and whether
/// the container is a throwaway or the build environment itself.
pub enum ContainerSignMode<'a> {
    /// A minimal, throwaway container: the output dir is mounted at
    /// [`OUTPUT_DIR_IN_CONTAINER`], always refresh apt, and fix ownership of
    /// the rewritten artifacts when the container root isn't id-mapped to the
    /// host user (docker).
    Ephemeral { chown_to: Option<(u32, u32)> },
    /// The build container itself: the `.changes` sits where
    /// `dpkg-buildpackage` wrote it, the keyring material is at `staging_dir`,
    /// and apt work is skipped when debsign is already installed.
    Build {
        work_dir: &'a Path,
        staging_dir: &'a Path,
    },
}

/// Shell scripts run inside a container to sign `changes_filename`. `setup`
/// links the forwarded agent socket, installs debsign and prepares the
/// keyring; `sign` invokes `debsign`. Runs as root.
pub fn sign_container_scripts(
    mode: ContainerSignMode,
    agent_socket: &Path,
    changes_filename: &str,
    sign_key: &str,
) -> SignContainerScripts {
    let sock = agent_socket.display();
    let pubkey = PUBKEY_FILE;
    let ownertrust = OWNERTRUST_FILE;
    let key = shell_single_quote(sign_key);
    let changes = shell_single_quote(changes_filename);

    let (staging, out, chown, install) = match mode {
        ContainerSignMode::Ephemeral { chown_to } => {
            let chown = match chown_to {
                Some((uid, gid)) => format!(
                    " && chown -R {uid}:{gid} {out}",
                    out = OUTPUT_DIR_IN_CONTAINER
                ),
                None => String::new(),
            };
            // A fresh container: always refresh apt. No -qq — the steps take
            // seconds and silence looks like a hang.
            let install = "echo 'debmagic: updating apt package lists'; \
                 apt-get update; \
                 echo 'debmagic: installing debsign'; \
                 apt-get install -y --no-install-recommends debsign \
                   || { echo 'debmagic: debsign package unavailable, installing devscripts instead'; \
                       apt-get install -y --no-install-recommends devscripts; }"
                .to_string();
            (
                SIGN_STAGING_IN_CONTAINER.to_string(),
                OUTPUT_DIR_IN_CONTAINER.to_string(),
                chown,
                install,
            )
        }
        ContainerSignMode::Build {
            work_dir,
            staging_dir,
        } => {
            // The build container already holds an apt cache; only touch apt
            // when debsign is genuinely missing.
            let install = "command -v debsign >/dev/null \
                   || { echo 'debmagic: installing debsign'; \
                       apt-get update; \
                       apt-get install -y --no-install-recommends debsign \
                         || apt-get install -y --no-install-recommends devscripts; }"
                .to_string();
            (
                staging_dir.display().to_string(),
                work_dir.display().to_string(),
                String::new(),
                install,
            )
        }
    };

    // gpg prefers the user-session socket dir (/run/user/$UID/gnupg) over
    // $GNUPGHOME, so the forwarded agent socket must be linked there; the
    // keyring (public key + ownertrust) still lives in $GNUPGHOME.
    let setup = format!(
        "set -e; \
         export GNUPGHOME=/root/.gnupg; \
         mkdir -p /run/user/0/gnupg \"$GNUPGHOME\"; \
         chmod 700 /run/user/0/gnupg \"$GNUPGHOME\"; \
         ln -sf {sock} /run/user/0/gnupg/S.gpg-agent; \
         ln -sf {sock} \"$GNUPGHOME/S.gpg-agent\"; \
         {install}; \
         gpg --batch --import {staging}/{pubkey}; \
         gpg --batch --import-ownertrust {staging}/{ownertrust}"
    );

    let sign = format!(
        "set -e; \
         export GNUPGHOME=/root/.gnupg; \
         cd {out} && debsign -k{key} {changes}{chown}"
    );

    SignContainerScripts { setup, sign }
}

/// Send a desktop notification via `notify-send`, if available. Never fails
/// the build: a headless session or missing binary just means no popup.
pub fn notify_send(summary: &str, body: &str) {
    match Command::new("notify-send")
        .arg(summary)
        .arg(body)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("notify-send not found on PATH; cannot send signing notification");
        }
        Err(e) => eprintln!("failed to run notify-send: {e}"),
    }
}

/// Sign `changes_file` on the host with `debsign`.
pub fn sign_on_host(
    changes_file: &Path,
    sign_key: Option<&str>,
    sign_notify: bool,
    package: &str,
) -> anyhow::Result<()> {
    let output_dir = changes_file.parent().ok_or_else(|| {
        anyhow!(
            "could not get output directory of {}",
            changes_file.display()
        )
    })?;
    let filename = changes_file
        .file_name()
        .ok_or_else(|| anyhow!("could not get filename of {}", changes_file.display()))?;

    let mut cmd = Command::new("debsign");
    if let Some(key) = sign_key {
        cmd.arg(format!("-k{key}"));
    }
    cmd.arg(filename).current_dir(output_dir);
    if sign_notify {
        notify_send(
            "debmagic: signing requested",
            &format!("touch your key to sign {package}"),
        );
    }
    run_checked(&mut cmd, &format!("signing {}", changes_file.display()))?;
    Ok(())
}

pub fn check_host_debsign_available() -> anyhow::Result<()> {
    match Command::new("debsign")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(anyhow!(
            "debsign not found on PATH. It's shipped in the debsign package (devscripts on \
             older releases); install it and set up a gpg signing key, or set sign_with to \
             \"same\" with a container driver."
        )),
        Err(e) => Err(e).context("failed to check for debsign"),
    }
}

pub fn changes_filename(changes_file: &Path) -> anyhow::Result<&str> {
    changes_file
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow!("could not get filename of {}", changes_file.display()))
}

fn shell_single_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ephemeral_script_quotes_filename_and_key() {
        let scripts = sign_container_scripts(
            ContainerSignMode::Ephemeral { chown_to: None },
            Path::new("/debmagic-gpg/S.gpg-agent"),
            "pkg_1.0_amd64.changes",
            "me@example.com",
        );
        assert!(
            scripts
                .sign
                .contains("debsign -k'me@example.com' 'pkg_1.0_amd64.changes'")
        );
        assert!(scripts.sign.contains("cd /debmagic-output"));
        assert!(
            scripts
                .setup
                .contains("gpg --batch --import /debmagic-sign/pubkey.asc")
        );
        // Ephemeral always refreshes apt and links the session socket.
        assert!(scripts.setup.contains("apt-get update"));
        assert!(scripts.setup.contains("/run/user/0/gnupg/S.gpg-agent"));
        assert!(!scripts.setup.contains("chown"));
        assert!(!scripts.sign.contains("chown"));
    }

    #[test]
    fn ephemeral_script_chowns_output_when_requested() {
        let scripts = sign_container_scripts(
            ContainerSignMode::Ephemeral {
                chown_to: Some((1000, 100)),
            },
            Path::new("/debmagic-gpg/S.gpg-agent"),
            "x.changes",
            "key",
        );
        assert!(scripts.sign.contains("chown -R 1000:100 /debmagic-output"));
        assert!(!scripts.setup.contains("chown"));
    }

    #[test]
    fn build_script_signs_in_place_and_skips_apt_when_present() {
        let scripts = sign_container_scripts(
            ContainerSignMode::Build {
                work_dir: Path::new("/debmagic/work"),
                staging_dir: Path::new("/tmp/debmagic-sign"),
            },
            Path::new("/tmp/debmagic-gpg/S.gpg-agent"),
            "pkg_1.0_amd64.changes",
            "me@example.com",
        );
        assert!(
            scripts
                .sign
                .contains("cd /debmagic/work && debsign -k'me@example.com'")
        );
        assert!(
            scripts
                .setup
                .contains("gpg --batch --import /tmp/debmagic-sign/pubkey.asc")
        );
        assert!(scripts.setup.contains("command -v debsign"));
        // The session socket dir is linked in both modes.
        assert!(scripts.setup.contains("/run/user/0/gnupg/S.gpg-agent"));
        assert!(!scripts.sign.contains("chown"));
    }

    #[test]
    fn shell_quote_escapes_single_quotes() {
        assert_eq!(shell_single_quote("a'b"), "'a'\\''b'");
    }
}
