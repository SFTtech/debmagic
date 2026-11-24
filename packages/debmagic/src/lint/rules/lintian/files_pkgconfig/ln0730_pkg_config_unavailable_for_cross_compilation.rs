use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct PkgConfigUnavailableForCrossCompilation;

declare_rule! {
    /// A pkg-config file is installed directly into `usr/lib/pkgconfig`.
    ///
    /// Walks the Data files that are regular files (hardlinks and symlinks
    /// are not) and reports each `usr/lib/pkgconfig/<name>.pc` at its path,
    /// with an empty extra. Udebs are checked too.
    PkgConfigUnavailableForCrossCompilation,
    code = "LN0730",
    tag = "pkg-config-unavailable-for-cross-compilation",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PkgConfigUnavailableForCrossCompilation {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<PathBuf> = files
            .query()
            .kinds([EntryKind::RegularFile])
            .into_iter()
            .filter(|entry| entry.path().to_str().is_some_and(is_unavailable_pkgconfig))
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in hits {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// `usr/lib/pkgconfig/<name>.pc`, with `<name>` a single component.
fn is_unavailable_pkgconfig(name: &str) -> bool {
    name.strip_prefix("usr/lib/pkgconfig/").is_some_and(|file| {
        file.len() > ".pc".len() && file.ends_with(".pc") && !file.contains('/')
    })
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn pc() -> TestFile {
        TestFile::new("Name: foo\n")
    }

    #[test]
    fn unavailable_pkgconfig_paths() {
        assert!(is_unavailable_pkgconfig("usr/lib/pkgconfig/libfoo.pc"));
        assert!(is_unavailable_pkgconfig("usr/lib/pkgconfig/x.pc"));
        assert!(!is_unavailable_pkgconfig("usr/lib/pkgconfig/.pc"));
        assert!(!is_unavailable_pkgconfig("usr/lib/pkgconfig/libfoo.pc.in"));
        assert!(!is_unavailable_pkgconfig("usr/lib/pkgconfig/sub/libfoo.pc"));
        assert!(!is_unavailable_pkgconfig("usr/lib/pkgconfig/"));
        assert!(!is_unavailable_pkgconfig(
            "usr/lib/x86_64-linux-gnu/pkgconfig/libfoo.pc"
        ));
        assert!(!is_unavailable_pkgconfig("usr/share/pkgconfig/libfoo.pc"));
        assert!(!is_unavailable_pkgconfig("usr/lib/libfoo.pc"));
        assert!(!is_unavailable_pkgconfig("usr/lib/pkgconfig2/libfoo.pc"));
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            BinaryPackageCase::from(vec![("usr/share/pkgconfig/libfoo.pc", pc())]),
            BinaryPackageCase::from(vec![("usr/lib/x86_64-linux-gnu/pkgconfig/libfoo.pc", pc())]),
            BinaryPackageCase::from(vec![("usr/lib/pkgconfig", TestFile::directory())]),
            BinaryPackageCase::from(vec![("usr/lib/pkgconfig/sub/libfoo.pc", pc())]),
            BinaryPackageCase::from(vec![("usr/lib/pkgconfig/libfoo.pc.in", pc())]),
            BinaryPackageCase::from(vec![(
                "usr/lib/pkgconfig/link.pc",
                TestFile::symlink("libfoo.pc"),
            )]),
            BinaryPackageCase::from(vec![("usr/lib/pkgconfig/hard.pc", pc().hardlink())]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![("usr/lib/pkgconfig/libfoo.pc", pc())]),
            BinaryPackageCase::from(vec![
                ("usr/lib/pkgconfig/a.pc", pc()),
                ("usr/lib/pkgconfig/b.pc", pc()),
            ]),
            BinaryPackageCase::from(vec![("usr/lib/pkgconfig/libfoo.pc", pc())]).udeb(),
        ];
        BinaryPackageTester::new(PkgConfigUnavailableForCrossCompilation, pass, fail)
            .test_and_snapshot();
    }
}
