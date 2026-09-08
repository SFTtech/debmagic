//! OpenPGP signing of build artifacts (`.changes`/`.dsc`/`.buildinfo`), as alternative to debsign.
//!
//! Signing always runs on the host with the host's gpg: the `.changes` and
//! its children are exported to the host output dir before signing, so no
//! agent forwarding or keyring seeding in containers is needed. Children are
//! signed first (`.dsc`, then `.buildinfo`), and after each child the
//! parent's checksums are rewritten.

use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use anyhow::{Context, bail};
use deb822_lossless::{Deb822, Paragraph};
use debian_control::pgp;
use md5::Md5;
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use sha2::{Digest, Sha256};

use crate::driver::SignRequest;
use crate::output::notify_send_bell;

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
    pub command: Option<String>,
}

/// Sign the `.changes` file (and its `.dsc`/`.buildinfo` children) on the
/// host, as resolved from the build's `sign` config.
pub fn sign_changes(request: &SignRequest) -> anyhow::Result<()> {
    let options = SignOptions {
        key: request.sign_key.map(str::to_string),
        tool: request.sign_tool,
        command: request.sign_command.map(str::to_string),
    };
    sign_file(
        request.changes_file,
        &options,
        request.notify,
        request.package,
    )
}

/// Read the single deb822 paragraph of a `.changes`/`.dsc`/`.buildinfo`
/// control file, losslessly, so checksum rewrites preserve the original
/// formatting of untouched fields byte-for-byte.
fn read_control(path: &Path) -> anyhow::Result<Paragraph> {
    let deb822 =
        Deb822::from_file(path).with_context(|| format!("failed to parse {}", path.display()))?;
    deb822
        .paragraphs()
        .next()
        .with_context(|| format!("{} contains no paragraph", path.display()))
}

fn write_control(paragraph: &Paragraph, path: &Path) -> anyhow::Result<()> {
    std::fs::write(path, paragraph.to_string())
        .with_context(|| format!("failed to write {}", path.display()))
}

/// Filenames ending in `.<ext>` listed under `Files:` or any
/// `Checksums-*:` field.
fn child_filename(paragraph: &Paragraph, ext: &str) -> Option<String> {
    let suffix = format!(".{ext}");
    for key in paragraph.keys() {
        if key != "Files" && !key.starts_with("Checksums-") {
            continue;
        }
        for line in paragraph
            .get(&key)
            .into_iter()
            .flat_map(|v| v.lines().map(str::to_string).collect::<Vec<_>>())
        {
            if let Some(name) = line.split_whitespace().next_back()
                && name.ends_with(&suffix)
            {
                return Some(name.to_string());
            }
        }
    }
    None
}

/// The hash algorithms used by the checksum fields of a control file.
#[derive(Clone, Copy)]
enum Hash {
    Md5,
    Sha1,
    Sha256,
}

impl Hash {
    fn hex(self, data: &[u8]) -> String {
        fn hex<D: Digest>(mut hasher: D, data: &[u8]) -> String {
            hasher.update(data);
            hasher
                .finalize()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect()
        }
        match self {
            Hash::Md5 => hex(Md5::new(), data),
            Hash::Sha1 => hex(Sha1::new(), data),
            Hash::Sha256 => hex(Sha256::new(), data),
        }
    }
}

/// The checksum fields debmagic understands, mapped to their hash.
const CHECKSUM_FIELDS: &[(&str, Hash)] = &[
    ("Files", Hash::Md5),
    ("Checksums-Sha1", Hash::Sha1),
    ("Checksums-Sha256", Hash::Sha256),
];

/// Rewrite one `Files:`/`Checksums-*:` line: the first token is the
/// checksum, the second the size, the last the filename; entries for
/// other files pass through unchanged.
fn rewrite_checksum_line(line: &str, filename: &str, checksum: &str, size: usize) -> String {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    match tokens.as_slice() {
        [old_checksum, old_size, middle @ .., name] if *name == filename => {
            let middle = if middle.is_empty() {
                String::new()
            } else {
                format!(" {}", middle.join(" "))
            };
            format!("{checksum} {size}{middle} {name}")
        }
        _ => line.to_string(),
    }
}

/// Rewrite the size and checksum entries for the file listings in a control file
fn fixup_checksums(paragraph: &mut Paragraph, filename: &str, data: &[u8]) -> anyhow::Result<()> {
    let size = data.len();

    for key in paragraph.keys() {
        if key.starts_with("Checksums-") && !CHECKSUM_FIELDS.iter().any(|(field, ..)| *field == key)
        {
            // An unknown checksum format would keep a stale checksum for a
            // re-signed file, producing an upload that fails verification
            // far away from here.
            bail!("unknown checksum field '{key}:' in control file");
        }
    }

    for (key, hash) in CHECKSUM_FIELDS {
        let Some(value) = paragraph.get(key) else {
            continue;
        };
        let checksum = hash.hex(data);
        let updated = value
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| rewrite_checksum_line(line, filename, &checksum, size))
            .collect::<Vec<_>>()
            .join("\n");
        paragraph.set(key, &updated);
    }
    Ok(())
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
/// `sign.command` argument, rejecting unknown or unterminated ones.
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
            .with_context(|| format!("unterminated '{{' in sign.command argument '{arg}'"))?;
        let name = &after[..end];
        let value = match name {
            "file" => file,
            "key" => key,
            "email" => match email {
                Some(email) => email,
                None => bail!("{{email}} used in sign.command but '{key}' contains no address"),
            },
            _ => bail!(
                "unknown placeholder '{{{name}}}' in sign.command (supported: {{file}}, {{key}}, {{email}})"
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
                .command
                .as_deref()
                .with_context(|| "sign.tool = \"custom\" requires sign.command to be set")?;
            let mut parts = command.split_whitespace();
            let program = parts.next().with_context(|| "sign.command is empty")?;
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
        cmd.args(["--output", "-", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped());
    } else {
        cmd.stdout(Stdio::piped());
    }

    let mut child = cmd
        .spawn()
        .with_context(|| format!("failed to run signing command for {}", path.display()))?;
    if pipes_stdin {
        use std::io::Write;
        child
            .stdin
            .take()
            .expect("stdin is piped")
            .write_all(&to_sign)
            .with_context(|| format!("failed to pipe {} to the signing command", path.display()))?;
    }
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
    output_dir: Option<&Path>,
) -> anyhow::Result<PathBuf> {
    let output_dir = output_dir.unwrap_or_else(|| Path::new(".."));
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
    fn child_filename_finds_dsc_and_buildinfo() {
        let control = parse_control(
            "Format: 1.8\nSource: pkg\nFiles:\n abc 123 pkg_1.0.dsc\n def 456 pkg_1.0.buildinfo\n ghi 789 other.txt\nChecksums-Sha256:\n xyz 123 pkg_1.0.dsc\n",
        );
        assert_eq!(
            child_filename(&control, "dsc").as_deref(),
            Some("pkg_1.0.dsc")
        );
        assert_eq!(
            child_filename(&control, "buildinfo").as_deref(),
            Some("pkg_1.0.buildinfo")
        );
        assert_eq!(child_filename(&control, "deb"), None);
    }

    #[test]
    fn fixup_rewrites_all_checksum_sections() {
        let mut control = parse_control(
            "Format: 1.8\nFiles:\n oldmd5 3 hash optional pkg_1.0.dsc\nChecksums-Sha1:\n oldsha1 3 pkg_1.0.dsc\nChecksums-Sha256:\n oldsha256 3 pkg_1.0.dsc\n",
        );
        let data = b"abc";
        fixup_checksums(&mut control, "pkg_1.0.dsc", data).unwrap();

        fn hex<D: Digest>(mut hasher: D, data: &[u8]) -> String {
            hasher.update(data);
            hasher
                .finalize()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect()
        }
        let md5 = hex(Md5::new(), data);
        let joined = control.to_string();
        assert!(joined.contains(&format!("{md5} 3 hash optional pkg_1.0.dsc")));
        assert!(joined.contains(&format!(" {} 3 pkg_1.0.dsc", hex(Sha1::new(), data))));
        assert!(joined.contains(&format!(" {} 3 pkg_1.0.dsc", hex(Sha256::new(), data))));
    }

    #[test]
    fn guess_signas_prefers_key_then_changed_by() {
        let options = SignOptions {
            key: Some("mykey".into()),
            tool: SignTool::Gpg,
            command: None,
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
