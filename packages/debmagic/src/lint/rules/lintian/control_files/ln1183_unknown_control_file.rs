use std::collections::HashSet;
use std::sync::LazyLock;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct UnknownControlFile;

declare_rule! {
    /// A regular control file is in neither permission table.
    ///
    /// A regular file of the Control files of a Binary package whose name is
    /// listed in neither `data/control-files/deb-permissions` nor
    /// `udeb-permissions` is reported as a warning. The extra is empty; the
    /// Location is the control file. Non-regular entries are reported by
    /// `control-file-is-not-a-file` instead, and a file listed for the other
    /// package type by `not-allowed-control-file`.
    UnknownControlFile,
    code = "LN1183",
    tag = "unknown-control-file",
    default_selected = true,
    experimental = false,
}

static DEB_FILES: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| names(include_str!("../../../data/control-files/deb-permissions")));
static UDEB_FILES: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| names(include_str!("../../../data/control-files/udeb-permissions")));

/// The first word of each line: the control file name, before its mode.
fn names(text: &'static str) -> HashSet<&'static str> {
    crate::lint::data::lines(text)
        .filter_map(|line| line.split_whitespace().next())
        .collect()
}

impl BinaryPackageRule for UnknownControlFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let (known, other) = if ctx.is_udeb() {
            (&*UDEB_FILES, &*DEB_FILES)
        } else {
            (&*DEB_FILES, &*UDEB_FILES)
        };
        let Some(files) = ctx.control_files() else {
            return;
        };
        let reports: Vec<Diagnostic> = files
            .entries()
            .iter()
            .filter(|entry| entry.is_regular_file())
            .filter_map(|entry| {
                let name = entry.path().to_str()?;
                (!known.contains(name) && !other.contains(name)).then(|| {
                    Diagnostic::warn(String::new()).with_location(Location {
                        path: entry.path().to_path_buf(),
                        line: None,
                        column: None,
                    })
                })
            })
            .collect();
        for diagnostic in reports {
            ctx.diagnostic(diagnostic);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn case(name: &str, file: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).control_file(name, file)
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            case("postinst", TestFile::new("#!/bin/sh\n")),
            case("md5sums", TestFile::new("")),
            // Non-regular entries are reported by the sibling Rule, not here.
            case("unknown", TestFile::symlink("postinst")),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .udeb()
                .control_file("isinstallable", TestFile::new("")),
        ];
        let fail = vec![
            case("special-file", TestFile::new("x")),
            case("unknown", TestFile::new("x")),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .udeb()
                .control_file("unknown", TestFile::new("x")),
        ];
        BinaryPackageTester::new(UnknownControlFile, pass, fail).test_and_snapshot();
    }
}
