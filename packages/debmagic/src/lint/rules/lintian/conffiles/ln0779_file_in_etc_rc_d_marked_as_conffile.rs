use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::declared::declared;

pub struct FileInEtcRcDMarkedAsConffile;

declare_rule! {
    /// A `conffiles` line names a path under `etc/rc?.d/`.
    ///
    /// The path (leading `/` removed) must match `etc/rc<any char>.d/`. Each
    /// such line is reported, duplicates included. Extra is the path; Location
    /// is `conffiles` at that line. Udebs are checked too.
    FileInEtcRcDMarkedAsConffile,
    code = "LN0779",
    tag = "file-in-etc-rc.d-marked-as-conffile",
    default_selected = true,
    experimental = false,
}

static RC_DIR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^etc/rc.\.d/").expect("valid regex"));

impl BinaryPackageRule for FileInEtcRcDMarkedAsConffile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for entry in declared(ctx) {
            if !RC_DIR.is_match(&entry.path) {
                continue;
            }
            ctx.diagnostic(Diagnostic::error(entry.path).with_location(Location {
                path: PathBuf::from("conffiles"),
                line: Some(entry.line),
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn conffiles(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file("conffiles", TestFile::new(body))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            conffiles("/etc/foo\n/etc/init.d/rc2.d\n"),
            conffiles("/etc/rc.d/file\n"),
            conffiles("/etc/rc2.d\n"),
            conffiles("/etc/rc10.d/file\n"),
            conffiles("remove-on-upgrade /usr/rc2.d/file\n"),
        ];
        let fail = vec![
            conffiles("/etc/rc2.d/file\n"),
            conffiles("etc/rcS.d/foo\n"),
            conffiles("/etc/foo\n\n/etc/rc3.d/x\n/etc/rc3.d/x\n"),
            conffiles("/etc/rc2.d/file\n").udeb(),
        ];
        BinaryPackageTester::new(FileInEtcRcDMarkedAsConffile, pass, fail).test_and_snapshot();
    }
}
