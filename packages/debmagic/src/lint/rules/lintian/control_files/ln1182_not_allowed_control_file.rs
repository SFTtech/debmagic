use std::collections::HashSet;
use std::sync::LazyLock;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct NotAllowedControlFile;

declare_rule! {
    /// A regular control file is one the other package type allows, not this one.
    ///
    /// A deb must not carry the control files only a udeb may have, and a udeb
    /// must not carry the ones only a deb may have, as listed in
    /// `data/control-files/deb-permissions` and `udeb-permissions`. Only
    /// regular files are judged here; the entry must be unknown to this
    /// package type but listed for the other one. The extra is empty; the
    /// Location is the control file.
    NotAllowedControlFile,
    code = "LN1182",
    tag = "not-allowed-control-file",
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

impl BinaryPackageRule for NotAllowedControlFile {
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
                (!known.contains(name) && other.contains(name)).then(|| {
                    Diagnostic::error(String::new()).with_location(Location {
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
            case("unknown", TestFile::new("x")),
            // `isinstallable` is listed for udeb only, and a hardlink is not judged.
            case("isinstallable", TestFile::new("").hardlink()),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .udeb()
                .control_file("isinstallable", TestFile::new("")),
        ];
        let fail = vec![
            case("isinstallable", TestFile::new("")),
            case("isinstallable", TestFile::new("#!/bin/sh\n")),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .udeb()
                .control_file("md5sums", TestFile::new("")),
        ];
        BinaryPackageTester::new(NotAllowedControlFile, pass, fail).test_and_snapshot();
    }
}
