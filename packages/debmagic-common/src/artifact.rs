//! Naming conventions of build and upload artifacts: orig tarballs,
//! `.changes` files and everything else a build or upload leaves behind.

/// Whether a `.changes` filename names a source-only upload:
/// `dpkg-buildpackage -S` writes `<name>_<version>_source.changes`, while
/// binary builds name the file after the build architecture instead.
pub fn is_source_changes(name: &str) -> bool {
    let Some(stem) = name.strip_suffix(".changes") else {
        return false;
    };
    stem.ends_with("_source")
}

/// Whether a filename is an `orig` tarball (main or component).
pub fn is_orig_tarball(name: &str) -> bool {
    // main: foo_1.0.orig.tar.gz; component: foo_1.0.orig-bar.tar.gz
    let Some((_, rest)) = name.split_once(".orig") else {
        return false;
    };
    if rest.starts_with(".tar.") {
        return true;
    }
    // component: the part between `.orig-` and `.tar.` is the component name
    match rest.strip_prefix('-').and_then(|r| r.split_once(".tar.")) {
        Some((component, _)) => !component.is_empty() && !component.contains('/'),
        None => false,
    }
}

/// Whether a filename looks like a build or upload artifact:
/// `<name>_<version>...` with a package, source, `.changes`, `.buildinfo`,
/// dput `.upload` or debmagic `.upload.json` suffix, or a tarball, possibly
/// with an `.asc` signature.
pub fn is_artifact_filename(name: &str) -> bool {
    const SUFFIXES: [&str; 9] = [
        ".changes",
        ".buildinfo",
        ".dsc",
        ".deb",
        ".udeb",
        ".ddeb",
        ".upload",
        ".upload.json",
        ".diff.gz",
    ];
    let name = name.strip_suffix(".asc").unwrap_or(name);
    name.contains('_')
        && (name.contains(".tar.") || SUFFIXES.iter().any(|suffix| name.ends_with(suffix)))
}

/// The `orig` tarball filename prefix for a source package version:
/// `<name>_<upstream>.orig.tar.` — the compression extension varies.
/// For a component, `<name>_<upstream>.orig-<component>.tar.`.
pub fn orig_prefix(name: &str, upstream_version: &str) -> String {
    format!("{name}_{upstream_version}.orig.tar.")
}

/// The `orig` tarball filename prefix for a component of a source
/// package version: `<name>_<upstream>.orig-<component>.tar.`.
pub fn component_orig_prefix(name: &str, upstream_version: &str, component: &str) -> String {
    format!("{name}_{upstream_version}.orig-{component}.tar.")
}

/// The possible `orig` tarball filenames of a source package version,
/// one per compression dpkg-source accepts (its `%COMP` table): the main
/// tarball, or a component's when `component` is set.
pub fn orig_filenames(name: &str, upstream_version: &str, component: Option<&str>) -> Vec<String> {
    let prefix = match component {
        None => orig_prefix(name, upstream_version),
        Some(component) => component_orig_prefix(name, upstream_version, component),
    };
    ["gz", "xz", "bz2", "lzma"]
        .iter()
        .map(|ext| format!("{prefix}{ext}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case("pkg_1.0.orig.tar.gz", true; "main orig")]
    #[test_case("pkg_1.0.orig.tar.xz", true; "main orig xz")]
    #[test_case("pkg_1.0.orig-bar.tar.xz", true; "component orig")]
    #[test_case("pkg_1.0-1.debian.tar.xz", false; "debian tarball")]
    #[test_case("pkg_1.0-1.dsc", false; "dsc")]
    #[test_case("pkg_1.0.orig.tar", false; "no compression ext")]
    fn test_is_orig_tarball(name: &str, expected: bool) {
        assert_eq!(is_orig_tarball(name), expected);
    }

    #[test_case("pkg_1.0-1_source.changes", true; "source upload")]
    #[test_case("pkg_1.0-1_amd64.changes", false; "binary upload")]
    #[test_case("pkg_1.0-1_all.changes", false; "arch-indep upload")]
    #[test_case("pkg_1.0-1_multi.changes", false; "multiarch upload")]
    #[test_case("pkg_1.0-1_amd64+all.changes", false; "combined upload")]
    #[test_case("pkg_1.0-1_amd64v3.changes", false; "arch variant upload")]
    #[test_case("pkg_1.0-1_source.buildinfo", false; "not a changes file")]
    fn test_is_source_changes(name: &str, expected: bool) {
        assert_eq!(is_source_changes(name), expected);
    }

    #[test_case("pkg_1.0.orig.tar.xz", true; "orig")]
    #[test_case("pkg_1.0.orig.tar.xz.asc", true; "orig signature")]
    #[test_case("pkg_1.0-1.debian.tar.xz", true; "debian tarball")]
    #[test_case("pkg_1.0.tar.xz", true; "native tarball")]
    #[test_case("pkg_1.0-1.dsc", true; "dsc")]
    #[test_case("pkg_1.0-1.diff.gz", true; "1.0 diff")]
    #[test_case("libpkg1_1.0-1_amd64.deb", true; "deb")]
    #[test_case("pkg-dbgsym_1.0-1_amd64.ddeb", true; "ddeb")]
    #[test_case("pkg_1.0-1_amd64.buildinfo", true; "buildinfo")]
    #[test_case("pkg_1.0-1_source.ppa.upload", true; "dput log")]
    #[test_case("pkg_1.0-1_source.upload.json", true; "debmagic upload record")]
    #[test_case("config.guess", false; "upstream script")]
    #[test_case("lib_shared.mk", false; "upstream makefile with underscore")]
    #[test_case("pkg.tar.gz", false; "tarball without version")]
    fn test_is_artifact_filename(name: &str, expected: bool) {
        assert_eq!(is_artifact_filename(name), expected);
    }

    #[test]
    fn test_orig_filenames() {
        assert_eq!(
            orig_filenames("pkg", "1.0", None),
            [
                "pkg_1.0.orig.tar.gz",
                "pkg_1.0.orig.tar.xz",
                "pkg_1.0.orig.tar.bz2",
                "pkg_1.0.orig.tar.lzma"
            ]
        );
        assert_eq!(
            orig_filenames("pkg", "1.0", Some("bar"))[0],
            "pkg_1.0.orig-bar.tar.gz"
        );
    }
}
