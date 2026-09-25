use std::path::Path;

use anyhow::Context;

use crate::sign::SignOptions;
use debmagic_common::package::SourcePackage;

/// Whether an upload should include the `orig` tarball.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, clap::ValueEnum)]
pub enum IncludeOrig {
    /// Include only when the archive cannot have this upstream
    /// version's orig yet: the changelog head bumps the upstream part
    /// relative to the entry below, or is a deltarebase onto a Debian
    /// version Ubuntu never had. Everything else — the usual
    /// Ubuntu-revision-on-top upload — the archive already carries
    /// the orig.
    #[default]
    Auto,
    Yes,
    No,
}

/// Resolve `--include-orig` against the changelog: `auto` includes
/// the orig only when the archive provably lacks it — a new upstream
/// version, or a deltarebase onto a Debian version Ubuntu never had.
/// For every other upload the archive already has it, so the default
/// is to not send it again.
pub fn decide_orig_upload(mode: IncludeOrig, source_dir: &Path) -> anyhow::Result<bool> {
    match mode {
        IncludeOrig::Yes => Ok(true),
        IncludeOrig::No => Ok(false),
        IncludeOrig::Auto => {
            let changelog = crate::changelog::load_changelog(source_dir)?;
            Ok(
                debmagic_common::debian::changelog::is_new_upstream_version(&changelog)
                    || debmagic_common::debian::changelog::is_deltarebase_onto_debian(&changelog),
            )
        }
    }
}

/// Apply an orig-inclusion decision to a `.changes` file: add or
/// remove the orig tarball entries (main and components), re-signing
/// with `sign_options` when the file changes.
pub fn changes_include_orig(
    changes_file: &Path,
    include: bool,
    package: &SourcePackage,
    sign_options: &SignOptions,
) -> anyhow::Result<()> {
    if package.is_native() {
        return Ok(());
    }
    let orig_names: Vec<String> = if include {
        let dir = changes_file
            .parent()
            .context("changes file has no parent directory")?;
        let mut names = Vec::new();
        if let Some(tarball) = debmagic_common::changes::find_orig_in_dir(
            dir,
            package.name(),
            package.version().upstream_version(),
        ) {
            names.push(tarball);
        }
        // component origs: any orig-<component> tarball next to the .changes
        for entry in dir
            .read_dir()
            .with_context(|| format!("failed to read {}", dir.display()))?
            .flatten()
        {
            let name = entry.file_name().to_string_lossy().into_owned();
            if debmagic_common::changes::is_orig_tarball(&name)
                && name.contains(".orig-")
                && name.starts_with(&format!(
                    "{}_{}.",
                    package.name(),
                    package.version().upstream_version()
                ))
            {
                names.push(entry.path());
            }
        }
        if names.is_empty() {
            anyhow::bail!(
                "no orig tarball for {} {} is next to {}",
                package.name(),
                package.version().upstream_version(),
                changes_file.display()
            );
        }
        for tarball in &names {
            verify_against_dsc(tarball, changes_file)?;
        }
        names
            .into_iter()
            .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .collect()
    } else {
        Vec::new()
    };

    let mut modified = if !include {
        crate::changes::set_changes_orig(changes_file, None)?
    } else {
        false
    };
    for name in &orig_names {
        modified |= crate::changes::set_changes_orig(changes_file, Some(name))?;
    }
    if modified {
        println!(
            "debmagic: re-signing {} after the orig tarball change",
            changes_file.display()
        );
        crate::sign::sign_file(changes_file, sign_options, false, package.name())?;
    }
    Ok(())
}

/// Verify the orig tarball against the checksums the `.dsc` referenced by
/// the `.changes` recorded for it — a mismatch means the tarball next to
/// the `.changes` is stale. Every checksum the `.dsc` records for the
/// tarball is checked; a missing `.dsc` or one listing no entry for the
/// tarball is skipped.
fn verify_against_dsc(tarball: &Path, changes_file: &Path) -> anyhow::Result<()> {
    let Some(dir) = changes_file.parent() else {
        return Ok(());
    };
    let changes = crate::control::read_changes(changes_file)?;
    let Some(files) = changes.files() else {
        return Ok(());
    };
    let Some(dsc_name) = files
        .iter()
        .find(|f| f.filename.ends_with(".dsc"))
        .map(|f| f.filename.clone())
    else {
        return Ok(());
    };
    let dsc_path = dir.join(&dsc_name);
    let dsc = crate::control::read_control(&dsc_path)
        .with_context(|| format!("failed to read {dsc_name}"))?;
    let tarball_name = tarball
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    let digests = crate::control::digest_file(tarball)?;

    debmagic_common::debian::control::verify_checksums(&dsc, &tarball_name, &digests).map_err(
        |error| {
            anyhow::anyhow!(
                "{error}; the tarball next to the .changes is stale (recorded by {dsc_name})"
            )
        },
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sha256_hex(data: &[u8]) -> String {
        use sha2::Digest;
        let mut hasher = sha2::Sha256::new();
        hasher.update(data);
        hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    #[test]
    fn test_verify_against_dsc() {
        let dir = std::env::temp_dir().join(format!("debmagic-dsc-verify-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let tarball = dir.join("pkg_1.0.orig.tar.xz");
        std::fs::write(&tarball, "tarball content").unwrap();
        let changes = dir.join("pkg_1.0-1_source.changes");
        std::fs::write(
            &changes,
            format!(
                "Format: 1.8\nFiles:\n {} 3 debiandev optional pkg_1.0-1.dsc\n",
                sha256_hex(b"dsc")
            ),
        )
        .unwrap();
        let dsc = dir.join("pkg_1.0-1.dsc");
        std::fs::write(
            &dsc,
            format!(
                "Format: 3.0 (quilt)\nChecksums-Sha256:\n {} 15 pkg_1.0.orig.tar.xz\n",
                sha256_hex(b"tarball content")
            ),
        )
        .unwrap();

        // matching checksum passes
        verify_against_dsc(&tarball, &changes).unwrap();

        // stale tarball fails
        std::fs::write(&tarball, "different content").unwrap();
        assert!(verify_against_dsc(&tarball, &changes).is_err());

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_resolve_include_orig_modes() {
        // Yes/No never consult any state; auto is covered by
        // test_resolve_include_orig_auto below
        let base = std::env::temp_dir().join(format!("debmagic-include-{}", std::process::id()));
        let source = base.join("pkg");
        std::fs::create_dir_all(source.join("debian")).unwrap();
        std::fs::write(
            source.join("debian").join("changelog"),
            "pkg (1.0-1) unstable; urgency=medium\n\n  * Change.\n\n -- A <a@example.com>  Mon, 01 Jan 2024 00:00:00 +0000\n",
        )
        .unwrap();

        assert!(decide_orig_upload(IncludeOrig::Yes, &source).unwrap());
        assert!(!decide_orig_upload(IncludeOrig::No, &source).unwrap());

        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn test_resolve_include_orig_auto() {
        // auto: a plain new Ubuntu revision — the archive already has
        // the orig, so it must not ride along
        let base =
            std::env::temp_dir().join(format!("debmagic-include-auto-{}", std::process::id()));
        let source = base.join("pkg");
        std::fs::create_dir_all(source.join("debian")).unwrap();
        std::fs::write(
            source.join("debian").join("changelog"),
            "pkg (1.0-1ubuntu2) unstable; urgency=medium\n\n  * Change.\n\n -- A <a@example.com>  Mon, 01 Jan 2024 00:00:00 +0000\n\n"
                .to_string()
                + "pkg (1.0-1ubuntu1) unstable; urgency=medium\n\n  * Change.\n\n -- A <a@example.com>  Mon, 01 Jan 2024 00:00:00 +0000\n",
        )
        .unwrap();
        assert!(!decide_orig_upload(IncludeOrig::Auto, &source).unwrap());

        // a deltarebase onto a Debian version: the archive cannot
        // have the orig yet, so it rides along
        std::fs::write(
            source.join("debian").join("changelog"),
            "pkg (1.0-1ubuntu1) unstable; urgency=medium\n\n  * Change.\n\n -- A <a@example.com>  Mon, 01 Jan 2024 00:00:00 +0000\n\n"
                .to_string()
                + "pkg (1.0-1) unstable; urgency=medium\n\n  * Change.\n\n -- A <a@example.com>  Mon, 01 Jan 2024 00:00:00 +0000\n",
        )
        .unwrap();
        assert!(decide_orig_upload(IncludeOrig::Auto, &source).unwrap());

        // a new upstream version: the archive cannot have its orig
        // yet either, so it rides along too
        std::fs::write(
            source.join("debian").join("changelog"),
            "pkg (1.1.0-1ubuntu1) unstable; urgency=medium\n\n  * Change.\n\n -- A <a@example.com>  Mon, 01 Jan 2024 00:00:00 +0000\n\n"
                .to_string()
                + "pkg (1.0.0-1ubuntu3) unstable; urgency=medium\n\n  * Change.\n\n -- A <a@example.com>  Mon, 01 Jan 2024 00:00:00 +0000\n",
        )
        .unwrap();
        assert!(decide_orig_upload(IncludeOrig::Auto, &source).unwrap());

        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn test_changes_include_orig_removes_entries() {
        let base =
            std::env::temp_dir().join(format!("debmagic-include-remove-{}", std::process::id()));
        let source = base.join("pkg");
        std::fs::create_dir_all(source.join("debian")).unwrap();
        std::fs::write(
            source.join("debian").join("changelog"),
            "pkg (1.0-1) unstable; urgency=medium\n\n  * Change.\n\n -- A <a@example.com>  Mon, 01 Jan 2024 00:00:00 +0000\n",
        )
        .unwrap();
        let changes = base.join("pkg_1.0-1_source.changes");
        std::fs::write(
            &changes,
            "Format: 1.8\nMaintainer: A <a@example.com>\nFiles:\n abc 3 debiandev optional pkg_1.0-1.dsc\nChecksums-Sha256:\n xyz 3 pkg_1.0-1.dsc\n",
        )
        .unwrap();
        std::fs::write(base.join("pkg_1.0.orig.tar.xz"), "tarball").unwrap();
        std::fs::write(base.join("pkg_1.0-1.dsc"), "Format: 3.0 (quilt)\n").unwrap();

        let package = crate::package::load_package(&source).unwrap();

        // a custom "signing" command that just cats the file back keeps
        // the test independent of a gpg keyring on the host
        let sign_options = SignOptions {
            tool: crate::sign::SignTool::Custom,
            sign_command: Some("cat".to_string()),
            ..Default::default()
        };

        // add: the orig entry appears
        changes_include_orig(&changes, true, &package, &sign_options).unwrap();
        let content = std::fs::read_to_string(&changes).unwrap();
        assert!(content.contains("pkg_1.0.orig.tar.xz"));

        // remove: the orig entry is gone again
        changes_include_orig(&changes, false, &package, &sign_options).unwrap();
        let content = std::fs::read_to_string(&changes).unwrap();
        assert!(!content.contains("orig.tar"));
        // the non-orig entries survive
        assert!(content.contains("pkg_1.0-1.dsc"));

        std::fs::remove_dir_all(&base).unwrap();
    }
}
