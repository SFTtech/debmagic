use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct StrayFolderInManual;

declare_rule! {
    /// A directory under `usr/share/man/` is not a manual section directory.
    ///
    /// Each directory whose path starts with `usr/share/man/` followed by a
    /// non-space character is checked against lintian's layout: the locale
    /// directory `usr/(X11R6|share)/man/` may hold `<locale>/` and `manN/`
    /// directories, and nothing else. Lintian matches the path with a trailing
    /// slash, so the same is done here. The directory is the Location, with a
    /// trailing slash; no extra. Udebs are skipped. Reported at error severity.
    StrayFolderInManual,
    code = "LN1357",
    tag = "stray-folder-in-manual",
    default_selected = true,
    experimental = false,
}

static MANPAGE_DIR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/share/man/\S+").expect("valid regex"));
static ALLOWED_DIR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^usr/(?:X11R6|share)/man/(?:[^/]+/)?(?:man\d/)?$").expect("valid regex")
});

impl BinaryPackageRule for StrayFolderInManual {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| entry.kind() == EntryKind::Directory)
            .filter_map(|entry| {
                let name = entry.path().to_str()?;
                if !MANPAGE_DIR.is_match(name) {
                    return None;
                }
                let with_slash = format!("{name}/");
                (!ALLOWED_DIR.is_match(&with_slash)).then(|| PathBuf::from(with_slash))
            })
            .collect();
        for path in paths {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn dirs(paths: &[&str]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            paths
                .iter()
                .map(|path| (path.to_string(), TestFile::directory()))
                .collect::<Vec<_>>(),
        )
        .field("Architecture", "all")
    }

    #[test]
    fn test() {
        let pass = vec![
            dirs(&[
                "usr/share/man/",
                "usr/share/man/man1",
                "usr/share/man/de",
                "usr/share/man/de/man1",
                "usr/X11R6/man/man3",
            ]),
            dirs(&["usr/share/doc/example"]),
        ];
        let fail = vec![
            dirs(&["usr/share/man/man1", "usr/share/man/de/misc"]),
            dirs(&["usr/share/man/de/man1/sub"]),
        ];
        BinaryPackageTester::new(StrayFolderInManual, pass, fail).test_and_snapshot();
    }
}
