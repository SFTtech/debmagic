use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct HeaderHasOverlyGenericName;

declare_rule! {
    /// A public header is named `util.h` or `utils.h`.
    ///
    /// `usr/include/` and one multiarch directory under it (`cpu-system-abi`,
    /// the shape of `DEB_HOST_MULTIARCH`). Other subdirectories are not
    /// public headers. The match ignores letter case. Extra is empty. The
    /// Location is the file.
    HeaderHasOverlyGenericName,
    code = "LN0393",
    tag = "header-has-overly-generic-name",
    default_selected = true,
    experimental = false,
}

static GENERIC: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^usr/include/(?:[a-z0-9_]+-[a-z0-9_]+-[a-z0-9_]+/)?utils?\.h$")
        .expect("valid regex")
});

impl BinaryPackageRule for HeaderHasOverlyGenericName {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink))
            .filter(|entry| {
                entry
                    .path()
                    .to_str()
                    .is_some_and(|path| GENERIC.is_match(path))
            })
            .map(|entry| entry.path().to_path_buf())
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

    fn files(paths: &[&str]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            paths
                .iter()
                .map(|path| (path.to_string(), TestFile::new("int x;\n")))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            files(&["usr/include/private-subdir/utils.h"]),
            files(&["usr/include/foo/utils.h"]),
            files(&["usr/include/myutils.h"]),
            files(&["usr/include/utils.h.bak"]),
        ];
        let fail = vec![
            files(&["usr/include/utils.h", "usr/include/util.h"]),
            files(&["usr/include/x86_64-linux-gnu/utils.h"]),
            files(&["usr/include/Utils.H"]),
        ];
        BinaryPackageTester::new(HeaderHasOverlyGenericName, pass, fail).test_and_snapshot();
    }
}
