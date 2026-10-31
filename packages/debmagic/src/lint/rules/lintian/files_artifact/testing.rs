use crate::lint::dsc::Dsc;
use crate::lint::tester::{SourcePackageCase, TestFile};

pub(super) fn control() -> (String, TestFile) {
    (
        "debian/control".to_string(),
        TestFile::new(
            "Source: example\n\
             Maintainer: Example <ex@example.com>\n\
             Standards-Version: 4.7.2\n\
             \n\
             Package: example\n\
             Architecture: all\n\
             Description: example\n extra\n",
        ),
    )
}

pub(super) fn file(path: &str) -> (String, TestFile) {
    (path.to_string(), TestFile::new("x"))
}

pub(super) fn text(path: &str, content: &str) -> (String, TestFile) {
    (path.to_string(), TestFile::new(content))
}

pub(super) fn dir(path: &str) -> (String, TestFile) {
    (path.to_string(), TestFile::directory())
}

pub(super) fn hardlink(path: &str) -> (String, TestFile) {
    (path.to_string(), TestFile::new("x").hardlink())
}

pub(super) fn symlink(path: &str) -> (String, TestFile) {
    (path.to_string(), TestFile::symlink("target"))
}

fn readme() -> (String, TestFile) {
    file("README")
}

pub(super) fn quilt_dsc() -> Dsc {
    Dsc::parse("Format: 3.0 (quilt)\nSource: example\n").expect("Dsc fixture")
}

pub(super) fn v1_dsc() -> Dsc {
    Dsc::parse("Format: 1.0\nSource: example\n").expect("Dsc fixture")
}

/// Non-native: `README` is in orig, `extra` is only in patched. No Dsc, so
/// the format is the missing-file default `1.0`.
pub(super) fn added(extra: Vec<(String, TestFile)>) -> SourcePackageCase {
    let mut patched = vec![readme()];
    patched.extend(extra);
    SourcePackageCase::from(patched).orig(vec![readme()])
}

/// Empty orig: native, so `debian-adds-` does not emit.
pub(super) fn native(extra: Vec<(String, TestFile)>) -> SourcePackageCase {
    SourcePackageCase::from(extra)
}

/// The same paths in orig and patched.
pub(super) fn in_orig(paths: Vec<(String, TestFile)>) -> SourcePackageCase {
    let mut both = vec![readme()];
    both.extend(paths);
    SourcePackageCase::from(both.clone()).orig(both)
}

/// Quilt via the Dsc `Format` field.
pub(super) fn quilt(extra: Vec<(String, TestFile)>) -> SourcePackageCase {
    added(extra).dsc(quilt_dsc())
}

/// Quilt via `debian/source/format` when the Dsc is omitted.
pub(super) fn quilt_format(extra: Vec<(String, TestFile)>) -> SourcePackageCase {
    let mut patched = vec![readme(), text("debian/source/format", "3.0 (quilt)\n")];
    patched.extend(extra);
    SourcePackageCase::from(patched).orig(vec![readme()])
}

/// Dsc `Format: 1.0` wins over a quilt `debian/source/format`.
pub(super) fn v1_dsc_over_quilt_format(extra: Vec<(String, TestFile)>) -> SourcePackageCase {
    let mut patched = vec![readme(), text("debian/source/format", "3.0 (quilt)\n")];
    patched.extend(extra);
    SourcePackageCase::from(patched)
        .orig(vec![readme()])
        .dsc(v1_dsc())
}
