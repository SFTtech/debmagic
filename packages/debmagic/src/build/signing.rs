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

use crate::build::common::run_checked;

/// Where the forwarded agent socket is bind-mounted inside sign containers.
/// A fixed, always-existing path; the script symlinks it to gpg's lookup
/// locations so plain `debsign` works without extra flags or env vars.
pub const GPG_SOCKET_IN_CONTAINER: &str = "/tmp/debmagic-gpg/S.gpg-agent";
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
    /// Sign inside a minimal container of the same distro, forwarding the
    /// host's gpg-agent socket. Requires `sign_key` to be set.
    Same,
}

/// Everything needed to GPG-sign inside a container: the host's gpg-agent
/// extra socket plus the public key to seed the container's keyring with.
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

/// Shell script run inside a sign container: install debsign, link the
/// forwarded agent socket where gpg looks for it, seed the throwaway keyring
/// with the public key plus ownertrust, then sign. Runs as root; `chown_to`
/// fixes ownership of the bind-mounted output dir afterwards when the
/// container's root is not id-mapped to the host user (docker).
pub fn sign_container_script(
    changes_filename: &str,
    sign_key: &str,
    chown_to: Option<(u32, u32)>,
) -> String {
    let chown = match chown_to {
        Some((uid, gid)) => format!(
            " && chown -R {uid}:{gid} {out}",
            out = OUTPUT_DIR_IN_CONTAINER
        ),
        None => String::new(),
    };
    format!(
        "set -e; \
         export GNUPGHOME=/root/.gnupg; \
         mkdir -p /run/user/0/gnupg \"$GNUPGHOME\"; \
         chmod 700 /run/user/0/gnupg \"$GNUPGHOME\"; \
         ln -sf {sock} /run/user/0/gnupg/S.gpg-agent; \
         ln -sf {sock} \"$GNUPGHOME/S.gpg-agent\"; \
         apt-get update -qq; \
         apt-get install -y -qq devscripts; \
         gpg --batch --import {staging}/{pubkey}; \
         gpg --batch --import-ownertrust {staging}/{ownertrust}; \
         cd {out} && debsign -k{key} {changes}{chown}",
        sock = GPG_SOCKET_IN_CONTAINER,
        staging = SIGN_STAGING_IN_CONTAINER,
        pubkey = PUBKEY_FILE,
        ownertrust = OWNERTRUST_FILE,
        out = OUTPUT_DIR_IN_CONTAINER,
        key = shell_single_quote(sign_key),
        changes = shell_single_quote(changes_filename),
    )
}

/// Sign `changes_file` on the host with `debsign`.
pub fn sign_on_host(changes_file: &Path, sign_key: Option<&str>) -> anyhow::Result<()> {
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
            "debsign not found on PATH. It's part of devscripts; install it and set up a \
             gpg signing key, or set sign_with to \"same\" with a container driver."
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
    fn sign_script_quotes_filename_and_key() {
        let script = sign_container_script("pkg_1.0_amd64.changes", "me@example.com", None);
        assert!(script.contains("debsign -k'me@example.com' 'pkg_1.0_amd64.changes'"));
        assert!(script.contains("gpg --batch --import /debmagic-sign/pubkey.asc"));
        assert!(!script.contains("chown"));
    }

    #[test]
    fn sign_script_chowns_output_when_requested() {
        let script = sign_container_script("x.changes", "key", Some((1000, 100)));
        assert!(script.contains("chown -R 1000:100 /debmagic-output"));
    }

    #[test]
    fn shell_quote_escapes_single_quotes() {
        assert_eq!(shell_single_quote("a'b"), "'a'\\''b'");
    }
}
