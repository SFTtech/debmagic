use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

pub struct DeprecatedConfigureFilename;

declare_rule! {
    /// The package has a `configure.in` at the top of its patched tree and a
    /// `Makefile.am` there too, so automake is in use with the filename
    /// deprecated since automake 1.14. The extra is empty and the Location is
    /// `configure.in`. Without a top-level `Makefile.am` nothing is reported,
    /// and a `configure.in` in a subdirectory is not looked at, as in lintian.
    DeprecatedConfigureFilename,
    code = "LN1511",
    tag = "deprecated-configure-filename",
    default_selected = false,
    experimental = false,
}

impl SourceRule for DeprecatedConfigureFilename {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        if files.get("Makefile.am").is_none() {
            return;
        }
        let Some(configure_in) = files.get("configure.in") else {
            return;
        };
        let path: PathBuf = configure_in.path().to_path_buf();
        ctx.diagnostic(
            Diagnostic::warn("")
                .with_severity(Severity::Info)
                .with_location(Location {
                    path,
                    line: None,
                    column: None,
                }),
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestCase};

    use super::*;

    #[test]
    fn source() {
        let pass = vec![
            // configure.ac is the current name.
            TestCase::from([("Makefile.am", ""), ("configure.ac", "")]),
            // No Makefile.am, so automake is not used.
            TestCase::from([("configure.in", "")]),
            // The files in a subdirectory are not the top-level ones.
            TestCase::from([("Makefile.am", ""), ("sub/configure.in", "")]),
            TestCase::from([("sub/Makefile.am", ""), ("configure.in", "")]),
        ];
        let fail = vec![
            TestCase::from([("Makefile.am", ""), ("configure.in", "")]),
            // Content does not matter.
            TestCase::from([
                ("Makefile.am", "all:\n"),
                ("configure.in", "AC_INIT\n"),
                ("configure.ac", ""),
            ]),
        ];
        SourceTreeTester::new(DeprecatedConfigureFilename, pass, fail).test_and_snapshot();
    }
}
