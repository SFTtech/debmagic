use std::path::{Path, PathBuf};

use anyhow::{Context, bail};

use crate::upload::{scp::ScpUploader, sftp::SftpUploader, target::substitute_placeholders};

pub mod orig;
pub mod scp;
pub mod sftp;
pub mod ssh;
pub mod target;

pub use target::{
    UploadConfig, UploadOverrides, UploadTarget, UploadTargetConfig, builtin_targets,
    parse_target_spec, resolve_target,
};

/// One upload method implementation. A new method (e.g. git-based)
/// is a new `UploadMethod` variant plus an implementation of this trait.
pub trait Uploader {
    /// Upload all `files` to the target's incoming dir, keeping each
    /// one's file name. Implementations should batch the files into a
    /// single session where the transport allows it.
    fn upload_files(&mut self, files: &[PathBuf]) -> anyhow::Result<()>;
}

/// The upload methods debmagic supports.
#[derive(
    Debug,
    Default,
    Copy,
    Clone,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    clap::ValueEnum,
)]
#[serde(rename_all = "snake_case")]
pub enum UploadMethod {
    /// `scp <file> <login@server>:<incoming>/<name>`
    #[default]
    Scp,
    /// `sftp -b` batch mode, `put`ting each file into `incoming`.
    Sftp,
}

/// Create the uploader for a target's configured method.
pub fn create_uploader(target: &UploadTarget) -> anyhow::Result<Box<dyn Uploader>> {
    let method = target.config.method.unwrap_or_default();
    match method {
        UploadMethod::Scp => Ok(Box::new(ScpUploader {
            target: target.clone(),
        })),
        UploadMethod::Sftp => Ok(Box::new(SftpUploader {
            target: target.clone(),
        })),
    }
}

/// Inputs for the `debmagic upload` command.
#[derive(Debug, Clone)]
pub struct UploadIntent {
    /// The package root, already resolved to an absolute path.
    pub source_dir: PathBuf,
    pub config_file: Option<PathBuf>,
    pub target: String,
    pub method: Option<UploadMethod>,
    pub server: Option<String>,
    pub incoming: Option<String>,
    pub login: Option<String>,
    pub port: Option<u16>,
    pub no_hooks: bool,
    pub force: bool,
    pub sign: Option<crate::sign::SignMode>,
    pub include_orig: Option<orig::IncludeOrig>,
    /// The explicit `.changes` to upload; located via changelog + output dir when unset.
    pub changes: Option<PathBuf>,
}

/// Run the `debmagic upload` command: resolve the target, optionally sign
/// the `.changes` and adjust its orig entries, then upload it.
pub fn upload(input: UploadIntent) -> anyhow::Result<()> {
    let source_dir = input.source_dir;
    let config = crate::config::Config::load(Some(&source_dir), input.config_file.as_deref())?;

    let mut upload_target = resolve_target(&input.target, Some(&config.upload))?;
    upload_target.apply_overrides(&UploadOverrides {
        method: input.method,
        server: input.server.clone(),
        incoming: input.incoming.clone(),
        login: input.login.clone(),
        port: input.port,
    });

    let identity = crate::package::load_package(&source_dir)?;
    let changes_file = match &input.changes {
        Some(file) => std::path::absolute(file).context("resolving the changes file failed")?,
        None => {
            let output_dir = std::path::absolute(source_dir.join(&config.output_dir))
                .context("resolving output dir failed")?;
            crate::sign::find_changes_file(
                identity.name(),
                &identity.version().to_string(),
                &output_dir,
            )?
        }
    };

    // adjust the orig entries before signing: rewriting the .changes
    // strips its signature, so signing afterwards touches the key once
    let orig_modified = match input.include_orig {
        Some(mode) => {
            let include = orig::decide_orig_upload(mode, &source_dir)?;
            orig::changes_include_orig(&changes_file, include, &identity)?
        }
        None => false,
    };

    let sign_mode = match input.sign {
        Some(mode) => mode,
        // the rewrite dropped the old signature; an unsigned upload is rejected
        None if orig_modified => crate::sign::SignMode::Auto,
        None => crate::sign::SignMode::No,
    };
    if sign_mode.is_enabled() {
        crate::output::stage("Signing");
        let package = changes_file
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        crate::sign::sign_file(
            &changes_file,
            &config.sign,
            sign_mode,
            config.sign.notify,
            &package,
        )?;
    }

    crate::output::stage(&format!(
        "Uploading {} to {}",
        changes_file.display(),
        upload_target.name
    ));
    upload_changes(
        &upload_target,
        &input.target,
        &changes_file,
        input.no_hooks,
        input.force,
        Some(identity.version().upstream_version()),
    )
    .with_context(|| {
        format!(
            "uploading {} to target '{}' failed",
            changes_file.display(),
            input.target
        )
    })
}

/// Read the local file names listed in a `.changes` file, plus
/// the `.changes` itself: everything that must be uploaded.
pub fn changes_upload_files(changes_path: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let changes = crate::control::read_changes(changes_path)?;
    let files = changes
        .files()
        .with_context(|| format!("{} has no Files field", changes_path.display()))?;
    let dir = changes_path
        .parent()
        .context("changes file has no parent directory")?;
    let mut paths: Vec<PathBuf> = Vec::with_capacity(files.len() + 1);
    for file in files {
        paths.push(dir.join(file.filename.clone()));
    }
    paths.push(changes_path.to_path_buf());
    Ok(paths)
}

/// Run a target's `pre_upload_commands`, each via `sh -c`, with the
/// `{changes}` placeholder substituted and `DEBMAGIC_UPLOAD_*` env vars
/// set. A non-zero exit aborts the upload before anything is transferred.
pub fn run_pre_upload_commands(
    target: &UploadTarget,
    target_spec: &str,
    changes_file: &Path,
    commands: &[String],
) -> anyhow::Result<()> {
    for command in commands {
        let command =
            substitute_placeholders(command, &[("changes", &changes_file.to_string_lossy())]);
        println!("debmagic: running pre-upload command: {command}");
        let status = std::process::Command::new("sh")
            .arg("-c")
            .arg(&command)
            .env("DEBMAGIC_UPLOAD_CHANGES", changes_file)
            .env("DEBMAGIC_UPLOAD_TARGET", target_spec)
            .env("DEBMAGIC_UPLOAD_TARGET_SERVER", &target.config.server)
            .env("DEBMAGIC_UPLOAD_TARGET_INCOMING", target.incoming_dir())
            .status()
            .with_context(|| format!("failed to run pre-upload command: {command}"))?;
        if !status.success() {
            bail!(
                "pre-upload command failed (exit status: {status}), aborting the upload: {command}"
            );
        }
    }
    Ok(())
}

/// Upload a `.changes` file and everything it references to a target:
/// run the pre-upload checks, then upload the referenced files and
/// the `.changes` itself. A successful upload is recorded in the
/// `.upload.json` next to the `.changes`; a prior successful upload
/// to the same target is refused unless `force` is set.
pub fn upload_changes(
    target: &UploadTarget,
    target_spec: &str,
    changes_file: &Path,
    skip_hooks: bool,
    force: bool,
    upstream_version: Option<&str>,
) -> anyhow::Result<()> {
    let mut log = UploadLog::load(changes_file)?;
    if !force && log.contains_target(target_spec) {
        bail!(
            "{target_spec} already has a successful upload of {} recorded in {}; pass --force to upload anyway",
            changes_file.display(),
            UploadLog::path_for(changes_file)?.display()
        );
    }

    let files = changes_upload_files(changes_file)?;

    if !skip_hooks {
        run_pre_upload_commands(
            target,
            target_spec,
            changes_file,
            &target.config.pre_upload_commands,
        )
        .context("running the pre-upload commands failed")?;
    }

    let mut uploader = create_uploader(target)?;
    uploader.upload_files(&files).with_context(|| {
        format!(
            "transferring {} files to {} failed",
            files.len(),
            target.name
        )
    })?;
    log.record(changes_file, target_spec, upstream_version)?;
    Ok(())
}

/// One completed upload, as recorded in the `.upload.json` file
/// next to the `.changes`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UploadRecord {
    /// The full target spec as invoked, e.g. `ppa:user/repo`.
    pub target: String,
    /// Local time of the completed upload, ISO 8601 with UTC offset
    /// (like python's `datetime.isoformat()`).
    pub time: String,
    /// The upstream version of the uploaded source
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_version: Option<String>,
}

/// The `.upload.json` file next to a `.changes`, holding the record of
/// every successful upload of it.
#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct UploadLog {
    pub uploads: Vec<UploadRecord>,
}

impl UploadLog {
    /// Path of the log for a `.changes` file:
    /// `<changes-stem>.upload.json` next to it.
    pub fn path_for(changes_file: &Path) -> anyhow::Result<PathBuf> {
        let stem = changes_file
            .file_stem()
            .context("changes file has no file stem")?;
        let dir = changes_file
            .parent()
            .context("changes file has no parent directory")?;
        Ok(dir.join(format!("{}.upload.json", stem.to_string_lossy())))
    }

    pub fn load(changes_file: &Path) -> anyhow::Result<Self> {
        let path = Self::path_for(changes_file)?;
        if !path.is_file() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        serde_json::from_str(&content)
            .with_context(|| format!("failed to parse {}", path.display()))
    }

    /// Was this exact target spec already uploaded successfully?
    pub fn contains_target(&self, target_spec: &str) -> bool {
        self.uploads.iter().any(|r| r.target == target_spec)
    }

    /// Append a record for a successful upload and write the file back.
    pub fn record(
        &mut self,
        changes_file: &Path,
        target_spec: &str,
        upstream_version: Option<&str>,
    ) -> anyhow::Result<()> {
        self.uploads.push(UploadRecord {
            target: target_spec.to_string(),
            time: chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
            upstream_version: upstream_version.map(str::to_string),
        });
        let path = Self::path_for(changes_file)?;
        let content = serde_json::to_string_pretty(self).context("serializing upload log")?;
        std::fs::write(&path, content)
            .with_context(|| format!("failed to write {}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upload_log_roundtrip_and_check() {
        let dir = std::env::temp_dir().join(format!("debmagic-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let changes = dir.join("pkg_1.0-1_source.changes");
        std::fs::write(&changes, "Format: 1.8\n").unwrap();

        let mut log = UploadLog::load(&changes).unwrap();
        assert!(!log.contains_target("ppa:sfttech/debmagic"));
        log.record(&changes, "ppa:sfttech/debmagic", Some("1.0"))
            .unwrap();
        assert!(log.contains_target("ppa:sfttech/debmagic"));
        // a different ppa is not a duplicate
        assert!(!log.contains_target("ppa:other/ppa"));

        // persists across loads
        let reloaded = UploadLog::load(&changes).unwrap();
        assert!(reloaded.contains_target("ppa:sfttech/debmagic"));
        assert_eq!(reloaded.uploads.len(), 1);
        // python isoformat-compatible, e.g. "2026-09-18T14:23:01+02:00"
        let time = &reloaded.uploads[0].time;
        assert_eq!(time.len(), 25);
        assert_eq!(&time[4..5], "-");
        assert_eq!(&time[10..11], "T");
        assert_eq!(&time[13..14], ":");
        assert!(time[19..].starts_with('+') || time[19..].starts_with('-'));

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
