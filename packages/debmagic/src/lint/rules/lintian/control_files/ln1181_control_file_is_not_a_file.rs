use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct ControlFileIsNotAFile;

declare_rule! {
    /// An entry of the Control files of a Binary package is not a regular file.
    ///
    /// Every entry of the control archive must be a regular file. A symlink,
    /// a hardlink, a directory, a device or a FIFO is reported, whatever its
    /// name. Lintian's `is_regular_file` does not count a hardlink, so a
    /// hardlink is reported here too. The extra is empty; the Location is the
    /// entry. The sibling Rules skip non-regular entries, as lintian does
    /// after this tag.
    ControlFileIsNotAFile,
    code = "LN1181",
    tag = "control-file-is-not-a-file",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for ControlFileIsNotAFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.control_files() else {
            return;
        };
        let reports: Vec<Diagnostic> = files
            .entries()
            .iter()
            .filter(|entry| !entry.is_regular_file())
            .map(|entry| {
                Diagnostic::error(String::new()).with_location(Location {
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
            case("unknown", TestFile::new("x")),
        ];
        let fail = vec![
            case("conffiles", TestFile::symlink("../../etc/passwd")),
            case("md5sums", TestFile::new("").hardlink()),
            case("special-file", TestFile::fifo()),
            case("postinst", TestFile::directory()),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .udeb()
                .control_file("config", TestFile::char_device()),
        ];
        BinaryPackageTester::new(ControlFileIsNotAFile, pass, fail).test_and_snapshot();
    }
}
