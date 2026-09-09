use std::path::{Path, PathBuf};

use anyhow::{Context, bail};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubjectKind {
    SourceTree,
    BinaryPackage,
    SourcePackage,
}

impl SubjectKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::SourceTree => "Source tree",
            Self::BinaryPackage => "Binary package",
            Self::SourcePackage => "Source package",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Subject {
    SourceTree(PathBuf),
    BinaryPackage(PathBuf),
    SourcePackage(PathBuf),
}

impl Subject {
    pub fn kind(&self) -> SubjectKind {
        match self {
            Self::SourceTree(_) => SubjectKind::SourceTree,
            Self::BinaryPackage(_) => SubjectKind::BinaryPackage,
            Self::SourcePackage(_) => SubjectKind::SourcePackage,
        }
    }

    pub fn path(&self) -> &Path {
        match self {
            Self::SourceTree(path) | Self::BinaryPackage(path) | Self::SourcePackage(path) => path,
        }
    }
}

pub fn resolve_subject(path: PathBuf) -> anyhow::Result<Subject> {
    let absolute = std::path::absolute(&path)
        .with_context(|| format!("resolving {} failed", path.display()))?;

    if absolute.is_dir() {
        return Ok(Subject::SourceTree(absolute));
    }

    match absolute.extension().and_then(|ext| ext.to_str()) {
        Some("deb" | "udeb" | "ddeb") => Ok(Subject::BinaryPackage(absolute)),
        Some("dsc") => Ok(Subject::SourcePackage(absolute)),
        _ => bail!(
            "cannot infer Subject from {}: expected a directory, .deb, .udeb, .ddeb, or .dsc",
            absolute.display()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn directory_resolves_to_source_tree_absolute() -> anyhow::Result<()> {
        let dir = std::env::temp_dir().join(format!("debmagic-subject-dir-{}", std::process::id()));
        fs::create_dir_all(&dir)?;
        let subject = resolve_subject(dir.clone())?;
        assert_eq!(subject.kind(), SubjectKind::SourceTree);
        assert_eq!(subject.path(), std::path::absolute(&dir)?);
        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn deb_extension_resolves_to_binary_package() -> anyhow::Result<()> {
        let path =
            std::env::temp_dir().join(format!("debmagic-subject-{}.deb", std::process::id()));
        fs::write(&path, b"")?;
        let subject = resolve_subject(path.clone())?;
        assert_eq!(subject.kind(), SubjectKind::BinaryPackage);
        assert_eq!(subject.path(), std::path::absolute(&path)?);
        let _ = fs::remove_file(&path);
        Ok(())
    }

    #[test]
    fn dsc_extension_resolves_to_source_package() -> anyhow::Result<()> {
        let path =
            std::env::temp_dir().join(format!("debmagic-subject-{}.dsc", std::process::id()));
        fs::write(&path, b"")?;
        let subject = resolve_subject(path.clone())?;
        assert_eq!(subject.kind(), SubjectKind::SourcePackage);
        assert_eq!(subject.path(), std::path::absolute(&path)?);
        let _ = fs::remove_file(&path);
        Ok(())
    }

    #[test]
    fn udeb_extension_resolves_to_binary_package() -> anyhow::Result<()> {
        let path =
            std::env::temp_dir().join(format!("debmagic-subject-{}.udeb", std::process::id()));
        fs::write(&path, b"")?;
        let subject = resolve_subject(path.clone())?;
        assert_eq!(subject.kind(), SubjectKind::BinaryPackage);
        assert_eq!(subject.path(), std::path::absolute(&path)?);
        let _ = fs::remove_file(&path);
        Ok(())
    }

    #[test]
    fn ddeb_extension_resolves_to_binary_package() -> anyhow::Result<()> {
        let path =
            std::env::temp_dir().join(format!("debmagic-subject-{}.ddeb", std::process::id()));
        fs::write(&path, b"")?;
        let subject = resolve_subject(path.clone())?;
        assert_eq!(subject.kind(), SubjectKind::BinaryPackage);
        assert_eq!(subject.path(), std::path::absolute(&path)?);
        let _ = fs::remove_file(&path);
        Ok(())
    }

    #[test]
    fn unknown_extension_errors() {
        let path =
            std::env::temp_dir().join(format!("debmagic-subject-{}.txt", std::process::id()));
        let error = resolve_subject(path).unwrap_err();
        assert!(error.to_string().contains("cannot infer Subject"));
    }
}
