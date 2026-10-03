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

use crate::config::SignConfig;
use crate::control::{child_filename, fixup_checksums, read_control, write_control};
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

/// Whether and how aggressively to sign. `keep` and `auto` sign what is
/// unsigned and leave files alone that our own key already signed; they
/// differ in what they do with a *foreign* signature — `keep` accepts
/// it, `auto` replaces it. `force` re-signs unconditionally (required
/// after a checksum rewrite invalidated the old signature).
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum SignMode {
    /// Never sign.
    No,
    /// Sign unsigned files; keep any existing signature, ours or not.
    Keep,
    /// Sign unsigned files; skip files our key already signed.
    #[default]
    Auto,
    /// Always strip an existing signature and sign again.
    Force,
}

impl SignMode {
    pub fn is_enabled(self) -> bool {
        self != Self::No
    }
}

/// How the unsigned content reaches the signing command.
#[derive(Debug, PartialEq, Eq)]
enum SignInput {
    /// Piped to the command's stdin (gpg and sq read `-`).
    Stdin,
    /// The file path is already an argument of the command.
    File,
}

/// One OpenPGP implementation's way of performing the three operations
/// debmagic needs: clearsigning, verifying a detached signature, and
/// telling whether a clearsigned file's signature is our own. The
/// invocation plumbing (running the command, reading stdout, error
/// reporting) is shared in this module; each impl only builds commands
/// and parses its tool's output.
trait SignBackend {
    /// The command that clearsigns `path`, and how the unsigned content
    /// reaches it.
    fn signing_command(&self, path: &Path, signas: &str) -> anyhow::Result<(Command, SignInput)>;

    /// The command that verifies `signature` over `file` against
    /// `keyring`; a non-zero exit means the verification failed.
    fn verify_command(
        &self,
        file: &Path,
        signature: &Path,
        keyring: &Path,
    ) -> anyhow::Result<Command>;

    /// Whether the clearsigned `path` was signed by the key `signas`
    /// resolves to. Whatever the backend cannot tell counts as "no" —
    /// callers then re-sign.
    fn signed_by_us(&self, path: &Path, signas: &str) -> bool;
}

/// Run `cmd` and return its captured stdout, or `None` when it cannot
/// be run — the same-key check then reports "not ours" and the caller
/// re-signs.
fn capture_stdout(cmd: Command) -> Option<String> {
    crate::subprocess::command(cmd)
        .capture(Capture::ALL)
        .run()
        .ok()
        .and_then(|result| result.stdout)
}

impl SignTool {
    fn backend(self, options: &SignConfig) -> Box<dyn SignBackend + '_> {
        match self {
            Self::Gpg => Box::new(Gpg),
            Self::Sequoia => Box::new(Sequoia),
            Self::Custom => Box::new(Custom { options }),
        }
    }
}

/// GnuPG's `gpg`.
struct Gpg;

impl SignBackend for Gpg {
    fn signing_command(&self, _path: &Path, signas: &str) -> anyhow::Result<(Command, SignInput)> {
        let mut cmd = Command::new("gpg");
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
            "--output",
            "-",
            "-",
        ]);
        Ok((cmd, SignInput::Stdin))
    }

    fn verify_command(
        &self,
        file: &Path,
        signature: &Path,
        keyring: &Path,
    ) -> anyhow::Result<Command> {
        let mut cmd = Command::new("gpg");
        cmd.args(["--no-default-keyring", "--keyring"])
            .arg(keyring)
            .arg("--verify")
            .arg(signature)
            .arg(file);
        Ok(cmd)
    }

    fn signed_by_us(&self, path: &Path, signas: &str) -> bool {
        // GOODSIG names the (sub)key that made the signature, while
        // VALIDSIG's last field is its primary key's fingerprint — the one
        // --list-secret-keys reports first. Our secret key is in our
        // keyring, so a signature gpg cannot verify is not ours.
        let mut verify = Command::new("gpg");
        verify.args(["--verify", "--status-fd=1"]).arg(path);
        let Some(status) = capture_stdout(verify) else {
            return false;
        };
        let Some(theirs) = status
            .lines()
            .find_map(|line| line.strip_prefix("[GNUPG:] VALIDSIG "))
            .and_then(|rest| rest.split_whitespace().nth(9))
        else {
            return false;
        };

        let mut list = Command::new("gpg");
        list.args(["--list-secret-keys", "--with-colons"])
            .arg(signas);
        let Some(colo) = capture_stdout(list) else {
            return false;
        };
        // The fpr field of the secret key's primary: fpr:<fingerprint>:...
        let Some(ours) = colo
            .lines()
            .find(|line| line.starts_with("fpr:"))
            .and_then(|line| line.split(':').nth(9))
        else {
            return false;
        };
        theirs.eq_ignore_ascii_case(ours)
    }
}

/// Sequoia's `sq`.
struct Sequoia;

impl SignBackend for Sequoia {
    fn signing_command(&self, _path: &Path, signas: &str) -> anyhow::Result<(Command, SignInput)> {
        let mut cmd = Command::new("sq");
        cmd.arg("sign").arg("--cleartext").arg("--mode=text");
        // sq selects the key by email, fingerprint or key ID.
        if signas.contains('@') {
            cmd.arg("--signer-email").arg(signas);
        } else {
            cmd.arg("--signer").arg(signas);
        }
        cmd.args(["--output", "-", "-"]);
        Ok((cmd, SignInput::Stdin))
    }

    fn verify_command(
        &self,
        file: &Path,
        signature: &Path,
        keyring: &Path,
    ) -> anyhow::Result<Command> {
        let mut cmd = Command::new("sq");
        cmd.arg("verify")
            .arg("--keyring")
            .arg(keyring)
            .arg(signature)
            .arg(file);
        Ok(cmd)
    }

    fn signed_by_us(&self, path: &Path, signas: &str) -> bool {
        // sq packet dump prints the signature's Issuer Fingerprint; our
        // side comes from inspecting the certificate signas selects.
        let mut dump = Command::new("sq");
        dump.args(["packet", "dump"]).arg(path);
        let Some(theirs) = capture_stdout(dump).and_then(|out| {
            out.lines()
                .find_map(|line| line.strip_prefix("Issuer Fingerprint: "))
                .map(str::trim)
                .map(str::to_string)
        }) else {
            return false;
        };

        let mut inspect = Command::new("sq");
        inspect.args(["inspect", "--cert-email"]).arg(signas);
        let Some(ours) = capture_stdout(inspect).and_then(|out| {
            out.lines()
                .find_map(|line| line.strip_prefix("Fingerprint: "))
                .map(str::trim)
                .map(str::to_string)
        }) else {
            return false;
        };
        theirs.eq_ignore_ascii_case(&ours)
    }
}

/// A custom command from `sign.sign_command`/`sign.verify_command`/
/// `sign.signed_by_command`, run without a shell.
struct Custom<'a> {
    options: &'a SignConfig,
}

impl Custom<'_> {
    /// Build `command` with the `{file}`, `{key}` and `{email}`
    /// placeholders substituted, appending the file path when no
    /// `{file}` placeholder is used.
    fn file_command(&self, command: &str, path: &Path, signas: &str) -> anyhow::Result<Command> {
        let mut parts = command.split_whitespace();
        let program = parts.next().context("the custom command is empty")?;
        let mut cmd = Command::new(program);
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
        Ok(cmd)
    }
}

impl SignBackend for Custom<'_> {
    fn signing_command(&self, path: &Path, signas: &str) -> anyhow::Result<(Command, SignInput)> {
        let command = self
            .options
            .sign_command
            .as_deref()
            .context("sign.tool = \"custom\" requires sign.sign_command to be set")?;
        Ok((self.file_command(command, path, signas)?, SignInput::File))
    }

    fn verify_command(
        &self,
        file: &Path,
        signature: &Path,
        keyring: &Path,
    ) -> anyhow::Result<Command> {
        let command = self
            .options
            .verify_command
            .as_deref()
            .or(self.options.sign_command.as_deref())
            .context(
                "sign.tool = \"custom\" requires sign.verify_command (or sign.sign_command) to be set",
            )?;
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
        Ok(cmd)
    }

    fn signed_by_us(&self, path: &Path, signas: &str) -> bool {
        // A custom tool cannot be introspected, but it can answer the
        // question itself: signed_by_command exits 0 when the file's
        // signature is ours. Without it, auto cannot detect same-key
        // signatures and re-signs.
        let Some(command) = self.options.signed_by_command.as_deref() else {
            return false;
        };
        let Ok(cmd) = self.file_command(command, path, signas) else {
            return false;
        };
        let Ok(result) = crate::subprocess::command(cmd).capture(Capture::ALL).run() else {
            return false;
        };
        result.exit_code == 0
    }
}

/// Verify a detached signature `signature` over `file` against the
/// keyring `keyring` (an exported OpenPGP key, like
/// `debian/upstream/signing-key.asc`), using the configured backend.
/// With `tool = "custom"`, `sign.command` runs with the `{file}`,
/// `{signature}` and `{keyring}` placeholders substituted; a command
/// without `{signature}` gets the signature path appended as the last
/// argument. A non-zero exit means the verification failed.
pub fn verify_signature(
    options: &SignConfig,
    file: &Path,
    signature: &Path,
    keyring: &Path,
) -> anyhow::Result<()> {
    let backend = options.tool.backend(options);
    let cmd = backend.verify_command(file, signature, keyring)?;
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

/// Sign the `.changes` file (and its `.dsc`/`.buildinfo` children) on the
/// host, as resolved from the build's `sign` config.
#[derive(Debug, Clone)]
pub struct SignIntent {
    pub source_dir: PathBuf,
    pub config_file: Option<PathBuf>,
    pub mode: SignMode,
    pub sign_key: Option<String>,
    pub sign_tool: Option<SignTool>,
    pub sign_command: Option<String>,
    pub sign_notify: Option<bool>,
    pub output_dir: Option<PathBuf>,
    /// The explicit file to sign; located via changelog + output dir when unset.
    pub file: Option<PathBuf>,
}

/// Run the `debmagic sign` command: resolve the config, locate the file
/// to sign and sign it.
pub fn sign(input: SignIntent) -> anyhow::Result<()> {
    let source_dir = input.source_dir;
    let mut config = crate::config::Config::load(Some(&source_dir), input.config_file.as_deref())?;

    if let Some(key) = &input.sign_key {
        config.sign.key = Some(key.clone());
    }
    if let Some(tool) = input.sign_tool {
        config.sign.tool = tool;
    }
    if let Some(command) = &input.sign_command {
        config.sign.sign_command = Some(command.clone());
    }
    if let Some(notify) = input.sign_notify {
        config.sign.notify = notify;
    }

    let options = SignConfig {
        key: config.sign.key.clone(),
        tool: config.sign.tool,
        sign_command: config.sign.sign_command.clone(),
        verify_command: config.sign.verify_command.clone(),
        signed_by_command: config.sign.signed_by_command.clone(),
        source: config.sign.source,
        notify: config.sign.notify,
    };

    let file = match &input.file {
        Some(file) => std::path::absolute(file).context("resolving the file to sign failed")?,
        None => {
            let identity = crate::package::load_package(&source_dir)?;
            // -o wins; else the config value, relative to the package root.
            let output_dir = match &input.output_dir {
                Some(dir) => std::path::absolute(dir).context("resolving output dir failed")?,
                None => std::path::absolute(source_dir.join(&config.output_dir))
                    .context("resolving output dir failed")?,
            };
            find_changes_file(
                identity.name(),
                &identity.version().to_string(),
                &output_dir,
            )?
        }
    };

    let package = file
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    sign_file(&file, &options, input.mode, config.sign.notify, &package)
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
fn guess_signas(options: &SignConfig, control: &Paragraph) -> String {
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
/// An existing signature is stripped first, so re-signing is idempotent.
fn sign_one(path: &Path, signas: &str, options: &SignConfig) -> anyhow::Result<()> {
    if is_signed(path)? {
        unsign(path)?;
    }
    let unsigned =
        std::fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut to_sign = unsigned;
    to_sign.push(b'\n');

    let (cmd, input) = options
        .tool
        .backend(options)
        .signing_command(path, signas)?;
    let mut builder = crate::subprocess::command(cmd).capture(Capture::STDOUT);
    if input == SignInput::Stdin {
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
///
/// `mode` decides whether an existing signature is kept: `auto` skips
/// files our own key already signed, `force` always re-signs. When any
/// child is (re)signed the parent is force-signed regardless, since its
/// checksums change and the old signature would no longer verify.
pub fn sign_file(
    path: &Path,
    options: &SignConfig,
    mode: SignMode,
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
            if !should_sign(&child, &signas, options, mode)? {
                continue;
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
    // Only rewrite the parent when a child changed: write_control would
    // strip the armor and destroy an existing signature we are keeping.
    if !signed.is_empty() {
        write_control(&control, path)?;
    }

    // A rewritten parent invalidates its old signature, so force-sign it
    // whenever a child changed; otherwise honor the mode.
    let parent_mode = if signed.is_empty() {
        mode
    } else {
        SignMode::Force
    };
    if should_sign(path, &signas, options, parent_mode)? {
        sign_one(path, &signas, options)?;
        println!("debmagic: signed {}", path.display());
    }
    Ok(())
}

/// Whether `path` should be (re-)signed under `mode`: unsigned files
/// always are, and a signed file only when `force` says so or the
/// signature is not ours.
fn should_sign(
    path: &Path,
    signas: &str,
    options: &SignConfig,
    mode: SignMode,
) -> anyhow::Result<bool> {
    if !is_signed(path)? {
        return Ok(true);
    }
    match mode {
        SignMode::No => Ok(false),
        SignMode::Keep => {
            println!("debmagic: {} is already signed; keeping", path.display());
            Ok(false)
        }
        SignMode::Force => {
            println!("debmagic: {} is already signed; re-signing", path.display());
            Ok(true)
        }
        SignMode::Auto => {
            if options.tool.backend(options).signed_by_us(path, signas) {
                println!(
                    "debmagic: {} is already signed with this key; skipping",
                    path.display()
                );
                Ok(false)
            } else {
                println!("debmagic: {} is already signed; re-signing", path.display());
                Ok(true)
            }
        }
    }
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
        let options = SignConfig {
            key: Some("mykey".into()),
            tool: SignTool::Gpg,
            sign_command: None,
            verify_command: None,
            signed_by_command: None,
            source: SignMode::No,
            notify: false,
        };
        let control =
            parse_control("Maintainer: A <a@example.com>\nChanged-By: B <b@example.com>\n");
        assert_eq!(guess_signas(&options, &control), "mykey");

        let options = SignConfig::default();
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

        let ok = SignConfig {
            verify_command: Some("true {signature}".to_string()),
            tool: SignTool::Custom,
            ..Default::default()
        };
        assert!(verify_signature(&ok, &file, &sig, &keyring).is_ok());

        let failing = SignConfig {
            verify_command: Some("false {signature}".to_string()),
            tool: SignTool::Custom,
            ..Default::default()
        };
        assert!(verify_signature(&failing, &file, &sig, &keyring).is_err());

        // verify_command falls back to sign_command
        let fallback = SignConfig {
            sign_command: Some("true {signature}".to_string()),
            tool: SignTool::Custom,
            ..Default::default()
        };
        assert!(verify_signature(&fallback, &file, &sig, &keyring).is_ok());

        // custom without any command configured is an error, not a gpg fallback
        let missing = SignConfig {
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
