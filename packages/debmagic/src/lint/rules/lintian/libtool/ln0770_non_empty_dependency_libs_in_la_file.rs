use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::ln0769_incorrect_libdir_in_la_file::{la_files, la_value};

pub struct NonEmptyDependencyLibsInLaFile;

declare_rule! {
    /// A libtool `.la` file still has a non-empty `dependency_libs` field.
    ///
    /// Walks the Data files that are regular files named `*.la` with valid
    /// UTF-8 content (symbolic links and hardlinks are skipped). Each
    /// `dependency_libs='...'` line with a non-empty value is reported at the
    /// line, with the value as the extra. Udebs are checked too.
    NonEmptyDependencyLibsInLaFile,
    code = "LN0770",
    tag = "non-empty-dependency_libs-in-la-file",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for NonEmptyDependencyLibsInLaFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut found: Vec<(PathBuf, u32, String)> = Vec::new();
        for (path, text) in la_files(files) {
            for (index, line) in text.split('\n').enumerate() {
                if let Some(prerequisites) = la_value(line, "dependency_libs") {
                    found.push((path.clone(), index as u32 + 1, prerequisites.to_string()));
                }
            }
        }
        for (path, line, extra) in found {
            ctx.diagnostic(Diagnostic::error(extra).with_location(Location {
                path,
                line: Some(line),
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn la(text: &str) -> TestFile {
        TestFile::new(text)
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            // An empty field is clear.
            BinaryPackageCase::from(vec![("usr/lib/libfoo.la", la("dependency_libs=''\n"))])
                .field("Architecture", "amd64"),
            // Other lines and files are not judged.
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.la",
                la("dependency_libs=-lm\n dependency_libs=' -lm'\n"),
            )])
            .field("Architecture", "amd64"),
            BinaryPackageCase::from(vec![("usr/lib/libfoo.so", la("dependency_libs=' -lm'\n"))])
                .field("Architecture", "amd64"),
            BinaryPackageCase::from(vec![("usr/lib/libfoo.la", TestFile::symlink("libbar.la"))])
                .field("Architecture", "amd64"),
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.la",
                la("dependency_libs=' -lm'\n").hardlink(),
            )])
            .field("Architecture", "amd64"),
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.la",
                TestFile::from_bytes(b"dependency_libs=' -lm'\n\xff".to_vec()),
            )])
            .field("Architecture", "amd64"),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![(
                "usr/lib/libdep.la",
                la("dependency_libs=' -lm -L/usr/lib'\n"),
            )])
            .field("Architecture", "amd64"),
            // Reported at the line of the field, one report per line.
            BinaryPackageCase::from(vec![(
                "usr/lib/libdep.la",
                la("# libtool\n\ndependency_libs='/usr/lib/libbar.la'\ndependency_libs='x'\n"),
            )])
            .field("Architecture", "amd64"),
            BinaryPackageCase::from(vec![("usr/lib/libdep.la", la("dependency_libs=' -lm'\n"))])
                .field("Architecture", "amd64")
                .udeb(),
        ];
        BinaryPackageTester::new(NonEmptyDependencyLibsInLaFile, pass, fail).test_and_snapshot();
    }
}
