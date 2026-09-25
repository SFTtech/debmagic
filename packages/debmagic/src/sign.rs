//! OpenPGP signing of build artifacts (`.changes`/`.dsc`/`.buildinfo`), as alternative to debsign.
//!
//! Signing always runs on the host with the host's gpg: the `.changes` and
//! its children are exported to the host output dir before signing, so no
//! agent forwarding or keyring seeding in containers is needed. Children are
//! signed first (`.dsc`, then `.buildinfo`), and after each child the
//! parent's checksums are rewritten.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, bail};
use deb822_lossless::Paragraph;
use debian_control::pgp;
use serde::{Deserialize, Serialize};

use crate::control::{child_filename, fixup_checksums, read_control, write_control};
use crate::driver::SignRequest;
use crate::output::notify_send_bell;
use crate::subprocess::Capture;

/// Which OpenPGP implementation performs the signing.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum SignTool {
    /// GnuPG's `gpg` (the default).
    #[default]
    Gpg,
    /// Sequoia's `sq`.
    Sequoia,
    /// A custom command from `sign.command`, run without a shell with the
    /// `{file}`, `{key}` and `{email}` placeholders substituted.
    Custom,
}

/// Verify a detached signature `signature` over `file` against the
/// keyring `keyring` (an exported OpenPGP key, like
/// `debian/upstream/signing-key.asc`), using the configured backend.
/// With `tool = "custom"`, `sign.command` runs with the `{file}`,
/// `{signature}` and `{keyring}` placeholders substituted; a command
/// without `{signature}` gets the signature path appended as the last
/// argument. A non-zero exit means the verification failed.
pub fn verify_signature(
    options: &SignOptions,
    file: &Path,
    signature: &Path,
    keyring: &Path,
) -> anyhow::Result<()> {
    let run = |cmd: Command| {
        let result = crate::subprocess::command(cmd)
            .capture(Capture::ALL)
            .run()
            .with_context(|| {
            format!(
                "failed to run the signature verification of {}",
                file.display()
            )
        })?;
        if result.exit_code != 0 {
            bail!(
                "signature verification of {} failed:\n{}",
                file.display(),
                result.stderr.as_deref().unwrap_or_default().trim()
            );
        }
        Ok(())
    };

    match options.tool {
        SignTool::Gpg => {
            let mut cmd = Command::new("gpg");
            cmd.args(["--no-default-keyring", "--keyring"])
                .arg(keyring)
                .arg("--verify")
                .arg(signature)
                .arg(file);
            run(cmd)
        }
        SignTool::Sequoia => {
            let mut cmd = Command::new("sq");
            cmd.arg("verify")
                .arg("--keyring")
                .arg(keyring)
                .arg(signature)
                .arg(file);
            run(cmd)
        }
        SignTool::Custom => {
            let command = options
                .verify_command
                .as_deref()
                .or(options.sign_command.as_deref())
                .context("sign.tool = \"custom\" requires sign.verify_command (or sign.sign_command) to be set")?;
            let mut parts = command.split_whitespace();
            let program = parts.next().context("the custom verify command is empty")?;
            let mut cmd = Command::new(program);
            let mut has_signature = false;
            for part in parts {
                has_signature |= part.contains("{signature}");
                cmd.arg(substitute_verify_placeholders(
                    part, file, signature, keyring,
                )?);
            }
            if !has_signature {
                cmd.arg(signature);
            }
            run(cmd)
        }
    }
}

/// Substitute the `{file}`, `{signature}` and `{keyring}` placeholders in
/// one custom verify-command argument, rejecting unknown or unterminated
/// ones.
fn substitute_verify_placeholders(
    arg: &str,
    file: &Path,
    signature: &Path,
    keyring: &Path,
) -> anyhow::Result<String> {
    let mut out = String::new();
    let mut rest = arg;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let end = after
            .find('}')
            .with_context(|| format!("unterminated '{{' in the verify command argument '{arg}'"))?;
        let name = &after[..end];
        let value = match name {
            "file" => file.display().to_string(),
            "signature" => signature.display().to_string(),
            "keyring" => keyring.display().to_string(),
            _ => bail!(
                "unknown placeholder '{{{name}}}' in the verify command (supported: {{file}}, {{signature}}, {{keyring}})"
            ),
        };
        out.push_str(&value);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

/// How to sign: which key to use, and which program does the OpenPGP work.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct SignOptions {
    /// GPG key ID/fingerprint/email to sign with. `None` falls back to the
    /// `Changed-By:`/`Maintainer:` address of the file being signed, which
    /// the signing tool matches against the keyring itself.
    pub key: Option<String>,
    /// Which OpenPGP implementation to use.
    pub tool: SignTool,
    /// Custom signing command when `tool` is [`SignTool::Custom`]. Run
    /// without a shell; `{file}`, `{key}` and `{email}` placeholders are
    /// substituted, and if no `{file}` is given the file path is appended
    /// as the last argument. The clearsigned result is read from stdout.
    pub sign_command: Option<String>,
    /// Custom verification command when `tool` is [`SignTool::Custom`],
    /// used by signature checks (e.g. `upstream switch`). Run without a
    /// shell; `{file}`, `{signature}` and `{keyring}` placeholders are
    /// substituted, and if no `{signature}` is given the signature path is
    /// appended as the last argument. Falls back to `sign_command`.
    pub verify_command: Option<String>,
}

/// Sign the `.changes` file (and its `.dsc`/`.buildinfo` children) on the
/// host, as resolved from the build's `sign` config.
pub fn sign_changes(request: &SignRequest) -> anyhow::Result<()> {
    let options = SignOptions {
        key: request.sign_key.map(str::to_string),
        tool: request.sign_tool,
        sign_command: request.sign_command.map(str::to_string),
        verify_command: None,
    };
    sign_file(
        request.changes_file,
        &options,
        request.notify,
        request.package,
    )
}

/// Is the file already clearsigned?
fn is_signed(path: &Path) -> anyhow::Result<bool> {
    let first = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?
        .lines()
        .next()
        .unwrap_or_default()
        .to_string();
    Ok(first == "-----BEGIN PGP SIGNED MESSAGE-----")
}

/// Strip an existing clearsign armor, leaving the plain message.
fn unsign(path: &Path) -> anyhow::Result<()> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let (mut payload, _) = pgp::strip_pgp_signature(&content)
        .with_context(|| format!("failed to strip the signature from {}", path.display()))?;
    // The armor's blank separator line before the signature is part of the
    // extracted payload; drop it so re-signing doesn't accumulate blank
    // lines.
    if payload.ends_with("\n\n") {
        payload.pop();
    }
    std::fs::write(path, payload).with_context(|| format!("failed to write {}", path.display()))
}

/// The key/user to sign as: explicit key, else the file's `Changed-By:` or
/// `Maintainer:` address.
fn guess_signas(options: &SignOptions, control: &Paragraph) -> String {
    if let Some(key) = &options.key {
        return key.clone();
    }
    control
        .get("Changed-By")
        .or_else(|| control.get("Maintainer"))
        .unwrap_or_default()
}

/// Substitute the `{file}`, `{key}` and `{email}` placeholders in one
/// `sign.sign_command` argument, rejecting unknown or unterminated ones.
fn substitute_placeholders(
    arg: &str,
    file: &str,
    key: &str,
    email: Option<&str>,
) -> anyhow::Result<String> {
    let mut out = String::new();
    let mut rest = arg;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let end = after
            .find('}')
            .with_context(|| format!("unterminated '{{' in sign.sign_command argument '{arg}'"))?;
        let name = &after[..end];
        let value = match name {
            "file" => file,
            "key" => key,
            "email" => match email {
                Some(email) => email,
                None => {
                    bail!("{{email}} used in sign.sign_command but '{key}' contains no address")
                }
            },
            _ => bail!(
                "unknown placeholder '{{{name}}}' in sign.sign_command (supported: {{file}}, {{key}}, {{email}})"
            ),
        };
        out.push_str(value);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

/// The bare address of a `Name <addr>` signer value, if any.
fn signer_email(signas: &str) -> Option<&str> {
    signas
        .split_whitespace()
        .find(|token| token.contains('@'))
        .map(|token| token.trim_matches(|c| c == '<' || c == '>'))
}

/// Clearsign `path` in place: the file gets a trailing
/// newline appended before signing, and the armored result replaces it.
fn sign_one(path: &Path, signas: &str, options: &SignOptions) -> anyhow::Result<()> {
    let unsigned =
        std::fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut to_sign = unsigned;
    to_sign.push(b'\n');

    let mut cmd = Command::new("gpg");
    // gpg and sq sign what is piped to stdin and write the armor to stdout;
    // a custom command receives the file path as an argument instead.
    let mut pipes_stdin = true;
    match options.tool {
        SignTool::Gpg => {
            cmd.args([
                "--no-auto-check-trustdb",
                "--local-user",
                signas,
                "--clearsign",
                "--openpgp",
                "--personal-digest-preferences",
                "SHA512 SHA384 SHA256 SHA224",
                "--list-options",
                "no-show-policy-urls",
                "--armor",
                "--textmode",
            ]);
        }
        SignTool::Sequoia => {
            cmd = Command::new("sq");
            cmd.arg("sign").arg("--cleartext").arg("--mode=text");
            // sq selects the key by email, fingerprint or key ID.
            if signas.contains('@') {
                cmd.arg("--signer-email").arg(signas);
            } else {
                cmd.arg("--signer").arg(signas);
            }
        }
        SignTool::Custom => {
            let command = options
                .sign_command
                .as_deref()
                .with_context(|| "sign.tool = \"custom\" requires sign.sign_command to be set")?;
            let mut parts = command.split_whitespace();
            let program = parts.next().with_context(|| "sign.sign_command is empty")?;
            cmd = Command::new(program);
            let file = path.display().to_string();
            let email = signer_email(signas);
            let mut has_file = false;
            for part in parts {
                has_file |= part.contains("{file}");
                cmd.arg(substitute_placeholders(part, &file, signas, email)?);
            }
            if !has_file {
                cmd.arg(file);
            }
            pipes_stdin = false;
        }
    }
    if pipes_stdin {
        cmd.args(["--output", "-", "-"]);
    }
    let mut builder = crate::subprocess::command(cmd).capture(Capture::STDOUT);
    if pipes_stdin {
        builder = builder.input(&to_sign);
    }
    let child = builder
        .spawn()
        .with_context(|| format!("failed to run the signing command for {}", path.display()))?;
    let output = child
        .wait_with_output()
        .with_context(|| format!("waiting for the signing command of {}", path.display()))?;
    if !output.status.success() {
        bail!(
            "signing {} failed (exit status: {}):\n{}",
            path.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    std::fs::write(path, output.stdout)
        .with_context(|| format!("failed to write signed {}", path.display()))?;
    Ok(())
}

/// Sign `path` (a `.changes`, `.buildinfo` or `.dsc` file) and, for a
/// `.changes`, its `.dsc`/`.buildinfo` children, rewriting the parent's
/// checksums after each child.
pub fn sign_file(
    path: &Path,
    options: &SignOptions,
    notify: bool,
    package: &str,
) -> anyhow::Result<()> {
    let dir = path
        .parent()
        .context("changes file has no parent directory")?;
    let mut control = read_control(path)?;

    let signas = guess_signas(options, &control);
    if signas.is_empty() {
        bail!(
            "no signing key configured and {} has no Changed-By/Maintainer to derive one from",
            path.display()
        );
    }
    if notify {
        notify_send_bell(
            "debmagic: signing requested",
            &format!("touch your key to sign {package}"),
        );
    }
    // Children first: .dsc, then .buildinfo.
    let mut signed = Vec::new();
    for ext in ["dsc", "buildinfo"] {
        if let Some(name) = child_filename(&control, ext) {
            let child = dir.join(&name);
            if !child.is_file() {
                bail!(
                    "{} references {} but it does not exist",
                    path.display(),
                    name
                );
            }
            if is_signed(&child)? {
                println!("debmagic: {name} is already signed; re-signing");
                unsign(&child)?;
            }
            sign_one(&child, &signas, options)?;
            println!("debmagic: signed {name}");
            signed.push((
                name,
                std::fs::read(&child).context("re-reading signed child")?,
            ));
        }
    }

    // Rewrite the parent's checksums for every (re)signed child.
    for (name, data) in &signed {
        fixup_checksums(&mut control, name, data)?;
    }
    write_control(&control, path)?;

    if is_signed(path)? {
        println!("debmagic: {} is already signed; re-signing", path.display());
        unsign(path)?;
    }
    sign_one(path, &signas, options)?;
    println!("debmagic: signed {}", path.display());
    Ok(())
}

/// Locate the `.changes` file to sign for the current source tree: any
/// `<package>_<version>_*.changes` in `output_dir`. When several match,
/// source-only (`_source`) is preferred, and the multiarch variants
/// (`_multi`, `_<a>+<b>`) are accepted too.
pub fn find_changes_file(
    package: &str,
    version: &str,
    output_dir: &Path,
) -> anyhow::Result<PathBuf> {
    let sversion = version
        .split_once(':')
        .map(|(_, rest)| rest)
        .unwrap_or(version);

    let pattern = output_dir.join(format!("{package}_{sversion}_*.changes"));
    let mut matches: Vec<PathBuf> = glob::glob(&pattern.to_string_lossy())
        .with_context(|| format!("invalid changes glob {}", pattern.display()))?
        .filter_map(Result::ok)
        .collect();
    matches.sort();

    let pick = matches
        .iter()
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().ends_with("_source.changes"))
        })
        .or_else(|| matches.first())
        .with_context(|| {
            format!(
                "could not find a .changes file for {package} {version} in {}",
                output_dir.display()
            )
        })?;
    Ok(pick.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_control(dir: &Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, content).unwrap();
        path
    }

    fn parse_control(content: &str) -> Paragraph {
        content.parse().unwrap()
    }

    #[test]
    fn guess_signas_prefers_key_then_changed_by() {
        let options = SignOptions {
            key: Some("mykey".into()),
            tool: SignTool::Gpg,
            sign_command: None,
            verify_command: None,
        };
        let control =
            parse_control("Maintainer: A <a@example.com>\nChanged-By: B <b@example.com>\n");
        assert_eq!(guess_signas(&options, &control), "mykey");

        let options = SignOptions::default();
        assert_eq!(guess_signas(&options, &control), "B <b@example.com>");

        let control = parse_control("Maintainer: A <a@example.com>\n");
        assert_eq!(guess_signas(&options, &control), "A <a@example.com>");
    }

    #[test]
    fn render_arg_substitutes_placeholders() {
        assert_eq!(
            substitute_placeholders("sign --key {key} {file}", "/tmp/f.dsc", "me", None).unwrap(),
            "sign --key me /tmp/f.dsc"
        );
        assert_eq!(
            substitute_placeholders("--sender={email}", "f", "A <a@b.c>", Some("a@b.c")).unwrap(),
            "--sender=a@b.c"
        );
        assert_eq!(
            substitute_placeholders("plain", "f", "k", None).unwrap(),
            "plain"
        );

        assert!(substitute_placeholders("{typo}", "f", "k", None).is_err());
        assert!(substitute_placeholders("{unterminated", "f", "k", None).is_err());
        assert!(substitute_placeholders("{email}", "f", "k", None).is_err());
    }

    #[test]
    fn signer_email_extracts_address() {
        assert_eq!(signer_email("B <b@example.com>"), Some("b@example.com"));
        assert_eq!(signer_email("b@example.com"), Some("b@example.com"));
        assert_eq!(signer_email("ABC1234"), None);
    }

    #[test]
    fn verify_placeholders_substitute() {
        assert_eq!(
            substitute_verify_placeholders(
                "verify --keyring {keyring} {file} {signature}",
                Path::new("/tmp/f.tar"),
                Path::new("/tmp/f.tar.asc"),
                Path::new("/tmp/key.asc"),
            )
            .unwrap(),
            "verify --keyring /tmp/key.asc /tmp/f.tar /tmp/f.tar.asc"
        );
        assert!(
            substitute_verify_placeholders(
                "{typo}",
                Path::new("f"),
                Path::new("s"),
                Path::new("k")
            )
            .is_err()
        );
        assert!(
            substitute_verify_placeholders(
                "{unterminated",
                Path::new("f"),
                Path::new("s"),
                Path::new("k")
            )
            .is_err()
        );
    }

    #[test]
    fn verify_signature_custom_runs_command() {
        // a custom command that exits 0 verifies; one that exits 1 fails
        let dir = std::env::temp_dir().join("debmagic-sign-test-verify");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("f.tar");
        std::fs::write(&file, "x").unwrap();
        let sig = dir.join("f.tar.asc");
        std::fs::write(&sig, "x").unwrap();
        let keyring = dir.join("key.asc");
        std::fs::write(&keyring, "x").unwrap();

        let ok = SignOptions {
            verify_command: Some("true {signature}".to_string()),
            tool: SignTool::Custom,
            ..Default::default()
        };
        assert!(verify_signature(&ok, &file, &sig, &keyring).is_ok());

        let failing = SignOptions {
            verify_command: Some("false {signature}".to_string()),
            tool: SignTool::Custom,
            ..Default::default()
        };
        assert!(verify_signature(&failing, &file, &sig, &keyring).is_err());

        // verify_command falls back to sign_command
        let fallback = SignOptions {
            sign_command: Some("true {signature}".to_string()),
            tool: SignTool::Custom,
            ..Default::default()
        };
        assert!(verify_signature(&fallback, &file, &sig, &keyring).is_ok());

        // custom without any command configured is an error, not a gpg fallback
        let missing = SignOptions {
            tool: SignTool::Custom,
            ..Default::default()
        };
        assert!(verify_signature(&missing, &file, &sig, &keyring).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unsign_strips_armor() {
        let dir = std::env::temp_dir().join("debmagic-sign-test-unsign");
        std::fs::create_dir_all(&dir).unwrap();
        let path = write_control(
            &dir,
            "f.dsc",
            "-----BEGIN PGP SIGNED MESSAGE-----\nHash: SHA512\n\nFormat: 3.0\n\n-----BEGIN PGP SIGNATURE-----\nsig\n-----END PGP SIGNATURE-----\n",
        );
        unsign(&path).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "Format: 3.0\n");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
