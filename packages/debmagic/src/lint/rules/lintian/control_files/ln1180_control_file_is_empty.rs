use std::collections::HashSet;
use std::sync::LazyLock;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct ControlFileIsEmpty;

declare_rule! {
    /// A recognised control file has no content.
    ///
    /// Checks the regular files of the Control files of a Binary package
    /// (deb and udeb) that `data/control-files/deb-permissions` (or
    /// `udeb-permissions` for a udeb) lists, when their content is zero
    /// bytes. `md5sums` is exempt: an empty `md5sums` is a package without
    /// files. Unknown names, hardlinks and other non-regular entries are not
    /// judged here. The `control` file is judged too. The extra is empty; the
    /// Location is the control file.
    ControlFileIsEmpty,
    code = "LN1180",
    tag = "control-file-is-empty",
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

impl BinaryPackageRule for ControlFileIsEmpty {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let known = if ctx.is_udeb() {
            &*UDEB_FILES
        } else {
            &*DEB_FILES
        };
        let Some(files) = ctx.control_files() else {
            return;
        };
        let reports: Vec<Diagnostic> = files
            .entries()
            .iter()
            .filter(|entry| entry.is_regular_file())
            .filter(|entry| entry.path().to_str() != Some("md5sums"))
            .filter(|entry| {
                entry
                    .path()
                    .to_str()
                    .is_some_and(|name| known.contains(name))
            })
            .filter(|entry| files.read(entry).is_ok_and(|content| content.is_empty()))
            .map(|entry| {
                Diagnostic::warn(String::new()).with_location(Location {
                    path: entry.path().to_path_buf(),
                    line: None,
                    column: None,
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
            case("unknown", TestFile::new("")),
            case("triggers", TestFile::new("").hardlink()),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .udeb()
                .control_file("md5sums", TestFile::new("")),
        ];
        let fail = vec![
            case("postinst", TestFile::new("")),
            case("triggers", TestFile::new("")),
            case("control", TestFile::new("")),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .udeb()
                .control_file("config", TestFile::new("")),
        ];
        BinaryPackageTester::new(ControlFileIsEmpty, pass, fail).test_and_snapshot();
    }
}
