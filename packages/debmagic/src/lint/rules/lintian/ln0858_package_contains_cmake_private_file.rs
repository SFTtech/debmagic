use std::path::PathBuf;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct PackageContainsCmakePrivateFile;

declare_rule! {
    /// The Binary package installs a regular file under `usr/share/cmake-N.N/`,
    /// a directory reserved for CMake's own modules.
    ///
    /// Walks Data files that are regular files or hardlinks, whose path matches
    /// `usr/share/cmake-<digits>.<digits>/`. Packages whose Source package is
    /// `cmake` are not hits. Extra is empty; the path is the Location.
    PackageContainsCmakePrivateFile,
    code = "LN0858",
    tag = "package-contains-cmake-private-file",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PackageContainsCmakePrivateFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.source_name() == "cmake" {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let pattern = Regex::new(r"^usr/share/cmake-\d+\.\d+/").expect("valid regex");
        let paths: Vec<PathBuf> = files
            .query()
            .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
            .into_iter()
            .filter(|entry| pattern.is_match(&entry.path().to_string_lossy()))
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

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            BinaryPackageCase::from(vec![(
                "usr/share/cmake-3.1/FindFoo.cmake",
                TestFile::new("x"),
            )])
            .field("Source", "cmake"),
            BinaryPackageCase::from(vec![(
                "usr/share/cmake/Foo/FooConfig.cmake",
                TestFile::new("x"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/cmake-3/FindFoo.cmake",
                TestFile::new("x"),
            )]),
            BinaryPackageCase::from(vec![("usr/share/cmake-3.1/Modules", TestFile::directory())]),
            BinaryPackageCase::from(vec![(
                "usr/share/cmake-3.1/FindFoo.cmake",
                TestFile::symlink("/usr/share/cmake/FindFoo.cmake"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/cmake-3.1/FindFoo.cmake",
                TestFile::new("x"),
            )]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![(
                "usr/share/cmake-3.1/FindFoo.cmake",
                TestFile::new("x"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/cmake-3.1/Modules/FindVar.cmake",
                TestFile::new("x"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/cmake-3.1/FindFoo.cmake",
                TestFile::new("x").hardlink(),
            )]),
            // A udeb is still a Binary package.
            BinaryPackageCase::from(vec![(
                "usr/share/cmake-3.1/FindFoo.cmake",
                TestFile::new("x"),
            )])
            .udeb(),
        ];
        BinaryPackageTester::new(PackageContainsCmakePrivateFile, pass, fail).test_and_snapshot();
    }
}
