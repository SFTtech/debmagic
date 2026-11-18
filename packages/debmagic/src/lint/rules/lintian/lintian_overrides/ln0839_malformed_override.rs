use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct MalformedOverride;

declare_rule! {
    /// A line in the Source override file is not a valid override.
    ///
    /// One Diagnostic per line of `debian/source/lintian-overrides` (else
    /// `debian/source.lintian-overrides`) that does not parse, with lintian's
    /// message as Extra, such as `Expected a colon` or `Cannot parse line: …`.
    /// The file and line are the Location. Installed override files are not
    /// checked. Overrides are not applied.
    MalformedOverride,
    code = "LN0839",
    tag = "malformed-override",
    default_selected = true,
    experimental = false,
}

impl SourceRule for MalformedOverride {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(overrides) = ctx.lintian_overrides() else {
            return;
        };
        for error in overrides.errors() {
            ctx.diagnostic(Diagnostic::error(error.message()).with_location(Location {
                path: overrides.path().to_path_buf(),
                line: Some(error.line()),
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "Source: example\n\
         Maintainer: Example <ex@example.com>\n\
         Standards-Version: 4.7.2\n\
         \n\
         Package: example\n\
         Architecture: all\n\
         Description: example\n extra\n";

    fn tree(overrides: &str) -> Vec<(String, TestFile)> {
        vec![
            ("debian/control".to_string(), TestFile::from(CONTROL)),
            (
                "debian/source/lintian-overrides".to_string(),
                TestFile::from(overrides),
            ),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("debian/control".to_string(), TestFile::from(CONTROL))],
            tree("example: no-copyright-file\n"),
            tree(
                "# why\nexample source: foo bar\n[amd64]: baz\nqux\n\nexample [amd64 !i386] source: x y\n",
            ),
            tree("other: foo\n"),
        ];
        let fail = vec![
            tree("example b\n"),
            tree("example:\n"),
            tree("example :  \n"),
            tree("example: ok\nexample c\n"),
            tree("example [amd64] foo\n"),
            tree("example udeb: baz\n"),
        ];
        SourceTreeTester::new(MalformedOverride, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree("example: foo\n"))];
        let fail = vec![SourcePackageCase::from(tree("example x\n"))];
        SourcePackageTester::new(MalformedOverride, pass, fail).test_and_snapshot();
    }
}
