use std::path::{Path, PathBuf};

use anyhow::{Context, bail};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubjectKind {
    SourceTree,
    BinaryPackage,
    SourcePackage,
    Changes,
    Buildinfo,
}

impl SubjectKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::SourceTree => "Source tree",
            Self::BinaryPackage => "Binary package",
            Self::SourcePackage => "Source package",
            Self::Changes => "Changes file",
            Self::Buildinfo => "Buildinfo file",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Subject {
    SourceTree(PathBuf),
    BinaryPackage(PathBuf),
    SourcePackage(PathBuf),
    Changes(PathBuf),
    Buildinfo(PathBuf),
}

impl Subject {
    pub fn kind(&self) -> SubjectKind {
        match self {
            Self::SourceTree(_) => SubjectKind::SourceTree,
            Self::BinaryPackage(_) => SubjectKind::BinaryPackage,
            Self::SourcePackage(_) => SubjectKind::SourcePackage,
            Self::Changes(_) => SubjectKind::Changes,
            Self::Buildinfo(_) => SubjectKind::Buildinfo,
        }
    }

    pub fn path(&self) -> &Path {
        match self {
            Self::SourceTree(path)
            | Self::BinaryPackage(path)
            | Self::SourcePackage(path)
            | Self::Changes(path)
            | Self::Buildinfo(path) => path,
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
        Some("changes") => Ok(Subject::Changes(absolute)),
        Some("buildinfo") => Ok(Subject::Buildinfo(absolute)),
        _ => bail!(
            "cannot infer Subject from {}: expected a directory, .deb, .udeb, .ddeb, .dsc, .changes, or .buildinfo",
            absolute.display()
        ),
    }
}

/// What one lint run inspects: one Source tree, or the Groups the artifacts
/// named on the command line make up (ADR 0054). A Source tree is never part
/// of a Group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LintTarget {
    SourceTree(PathBuf),
    /// One or more artifact paths, in argument order, none of them a Source
    /// tree. Grouping happens when the run opens them.
    Artifacts(Vec<Subject>),
}

impl LintTarget {
    /// The Subject kind of a single path, or `None` for several artifacts.
    #[allow(dead_code)]
    pub fn single_kind(&self) -> Option<SubjectKind> {
        match self {
            Self::SourceTree(_) => Some(SubjectKind::SourceTree),
            Self::Artifacts(subjects) => match subjects.as_slice() {
                [only] => Some(only.kind()),
                _ => None,
            },
        }
    }
}

/// Resolve the paths of a lint run. A directory is a Source tree and the only
/// path; otherwise every path is an artifact. The same path twice counts once.
pub fn resolve_target(paths: Vec<PathBuf>) -> anyhow::Result<LintTarget> {
    let mut subjects: Vec<Subject> = Vec::new();
    for path in paths {
        let subject = resolve_subject(path)?;
        if !subjects.contains(&subject) {
            subjects.push(subject);
        }
    }
    if let Some(Subject::SourceTree(tree)) = subjects
        .iter()
        .find(|subject| subject.kind() == SubjectKind::SourceTree)
    {
        if subjects.len() > 1 {
            bail!(
                "{} is a Source tree, which cannot be linted together with other paths",
                tree.display()
            );
        }
        return Ok(LintTarget::SourceTree(tree.clone()));
    }
    if subjects.is_empty() {
        bail!("nothing to lint: no path given");
    }
    Ok(LintTarget::Artifacts(subjects))
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
    fn changes_and_buildinfo_extensions_resolve() -> anyhow::Result<()> {
        for (name, kind) in [
            ("a.changes", SubjectKind::Changes),
            ("a.buildinfo", SubjectKind::Buildinfo),
        ] {
            let subject = resolve_subject(PathBuf::from(name))?;
            assert_eq!(subject.kind(), kind);
            assert!(subject.path().is_absolute());
        }
        Ok(())
    }

    #[test]
    fn several_artifacts_are_one_target_without_duplicates() -> anyhow::Result<()> {
        let target = resolve_target(vec![
            PathBuf::from("a_1.0_amd64.changes"),
            PathBuf::from("b_1.0_all.deb"),
            PathBuf::from("a_1.0_amd64.changes"),
        ])?;
        let LintTarget::Artifacts(subjects) = target else {
            panic!("expected artifacts");
        };
        let kinds: Vec<_> = subjects.iter().map(Subject::kind).collect();
        assert_eq!(kinds, [SubjectKind::Changes, SubjectKind::BinaryPackage]);
        Ok(())
    }

    #[test]
    fn a_source_tree_cannot_join_artifacts() -> anyhow::Result<()> {
        let dir = std::env::temp_dir();
        let error = resolve_target(vec![dir.clone(), PathBuf::from("a.deb")]).unwrap_err();
        assert!(error.to_string().contains("Source tree"), "{error}");
        let target = resolve_target(vec![dir.clone()])?;
        assert_eq!(target.single_kind(), Some(SubjectKind::SourceTree));
        Ok(())
    }

    #[test]
    fn no_path_is_an_error() {
        assert!(resolve_target(Vec::new()).is_err());
    }

    #[test]
    fn unknown_extension_errors() {
        let path =
            std::env::temp_dir().join(format!("debmagic-subject-{}.txt", std::process::id()));
        let error = resolve_subject(path).unwrap_err();
        assert!(error.to_string().contains("cannot infer Subject"));
    }
}
