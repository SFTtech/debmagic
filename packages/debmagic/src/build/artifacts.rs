use std::{
    ffi::OsStr,
    fs,
    path::{Component, Path, PathBuf},
};

use anyhow::{Context, anyhow, bail};
use debian_control::lossless::changes::Changes;

fn changes_file_in(build_dir: &Path) -> anyhow::Result<PathBuf> {
    let mut paths = fs::read_dir(build_dir)
        .with_context(|| {
            format!(
                "failed to read build output directory {}",
                build_dir.display()
            )
        })?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension() == Some(OsStr::new("changes")));

    let path = paths
        .next()
        .ok_or_else(|| anyhow!("build produced no .changes file in {}", build_dir.display()))?;
    if paths.next().is_some() {
        bail!(
            "build produced multiple .changes files in {}; refusing to guess which upload set to export",
            build_dir.display()
        );
    }
    Ok(path)
}

fn artifact_filename(filename: &str) -> anyhow::Result<&OsStr> {
    let path = Path::new(filename);
    let mut components = path.components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(filename)), None) => Ok(filename),
        _ => bail!("invalid artifact filename in .changes: {filename:?}"),
    }
}

fn reject_destination_symlink(path: &Path) -> anyhow::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            bail!("refusing to overwrite artifact symlink {}", path.display())
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

pub fn export_build_artifacts(build_dir: &Path, output_dir: &Path) -> anyhow::Result<PathBuf> {
    fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create output directory {}", output_dir.display()))?;

    let changes_path = changes_file_in(build_dir)?;
    let changes_metadata = fs::symlink_metadata(&changes_path)?;
    if !changes_metadata.file_type().is_file() {
        bail!(
            "changes file {} is not a regular file",
            changes_path.display()
        );
    }
    let changes = Changes::from_file(&changes_path)
        .with_context(|| format!("failed to parse {}", changes_path.display()))?;
    let files = changes
        .files()
        .ok_or_else(|| anyhow!("{} has no Files field", changes_path.display()))?;

    for file in files {
        let filename = artifact_filename(&file.filename)?;
        let source = build_dir.join(filename);
        let metadata = fs::symlink_metadata(&source).with_context(|| {
            format!(
                "artifact {} referenced by {} does not exist",
                source.display(),
                changes_path.display()
            )
        })?;
        if !metadata.file_type().is_file() {
            bail!("build artifact {} is not a regular file", source.display());
        }
        if metadata.len() != file.size as u64 {
            bail!(
                "artifact {} has size {}, but {} records {}",
                source.display(),
                metadata.len(),
                changes_path.display(),
                file.size
            );
        }
        let destination = output_dir.join(filename);
        reject_destination_symlink(&destination)?;
        fs::copy(&source, destination)
            .with_context(|| format!("failed to copy build artifact {}", source.display()))?;
    }

    let changes_filename = changes_path
        .file_name()
        .ok_or_else(|| anyhow!("invalid .changes path: {}", changes_path.display()))?;
    let exported_changes = output_dir.join(changes_filename);
    reject_destination_symlink(&exported_changes)?;
    fs::copy(&changes_path, &exported_changes)
        .with_context(|| format!("failed to copy {}", changes_path.display()))?;
    Ok(exported_changes)
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::symlink;

    use super::*;

    fn test_dir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("debmagic-{name}-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        path
    }

    fn write_changes(path: &Path, files: &[(&str, &str)]) {
        let files = files
            .iter()
            .map(|(filename, contents)| {
                fs::write(path.parent().unwrap().join(filename), contents).unwrap();
                format!(
                    " d41d8cd98f00b204e9800998ecf8427e {} misc optional {}",
                    contents.len(),
                    filename
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(
            path,
            format!("Format: 1.8\nSource: test\nFiles:\n{files}\n"),
        )
        .unwrap();
    }

    #[test]
    fn exports_every_file_listed_by_changes() {
        let build_dir = test_dir("artifact-build");
        let output_dir = test_dir("artifact-output");
        let changes_path = build_dir.join("test_1_amd64.changes");
        let artifacts = [
            ("test_1_amd64.deb", "deb"),
            ("test_1_amd64.udeb", "udeb"),
            ("test-dbgsym_1_amd64.ddeb", "ddeb"),
            ("test_1_amd64.buildinfo", "buildinfo"),
        ];
        write_changes(&changes_path, &artifacts);

        let exported_changes = export_build_artifacts(&build_dir, &output_dir).unwrap();

        assert_eq!(exported_changes, output_dir.join("test_1_amd64.changes"));
        for (filename, contents) in artifacts {
            assert_eq!(
                fs::read_to_string(output_dir.join(filename)).unwrap(),
                contents
            );
        }
        fs::remove_dir_all(build_dir).unwrap();
        fs::remove_dir_all(output_dir).unwrap();
    }

    #[test]
    fn rejects_ambiguous_changes_files() {
        let build_dir = test_dir("artifact-ambiguous");
        let output_dir = test_dir("artifact-ambiguous-output");
        fs::write(build_dir.join("one.changes"), "").unwrap();
        fs::write(build_dir.join("two.changes"), "").unwrap();

        let error = export_build_artifacts(&build_dir, &output_dir).unwrap_err();

        assert!(error.to_string().contains("multiple .changes files"));
        fs::remove_dir_all(build_dir).unwrap();
        fs::remove_dir_all(output_dir).unwrap();
    }

    #[test]
    fn rejects_artifact_symlinks() {
        let build_dir = test_dir("artifact-symlink");
        let output_dir = test_dir("artifact-symlink-output");
        let changes_path = build_dir.join("test_1_amd64.changes");
        write_changes(&changes_path, &[("test_1_amd64.deb", "deb")]);
        fs::remove_file(build_dir.join("test_1_amd64.deb")).unwrap();
        fs::write(build_dir.join("target"), "deb").unwrap();
        symlink("target", build_dir.join("test_1_amd64.deb")).unwrap();

        let error = export_build_artifacts(&build_dir, &output_dir).unwrap_err();

        assert!(error.to_string().contains("not a regular file"));
        fs::remove_dir_all(build_dir).unwrap();
        fs::remove_dir_all(output_dir).unwrap();
    }

    #[test]
    fn rejects_destination_symlinks() {
        let build_dir = test_dir("artifact-destination-symlink");
        let output_dir = test_dir("artifact-destination-symlink-output");
        let changes_path = build_dir.join("test_1_amd64.changes");
        write_changes(&changes_path, &[("test_1_amd64.deb", "deb")]);
        let target = output_dir.join("target");
        fs::write(&target, "unchanged").unwrap();
        symlink("target", output_dir.join("test_1_amd64.deb")).unwrap();

        let error = export_build_artifacts(&build_dir, &output_dir).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("refusing to overwrite artifact symlink")
        );
        assert_eq!(fs::read_to_string(target).unwrap(), "unchanged");
        fs::remove_dir_all(build_dir).unwrap();
        fs::remove_dir_all(output_dir).unwrap();
    }
}
