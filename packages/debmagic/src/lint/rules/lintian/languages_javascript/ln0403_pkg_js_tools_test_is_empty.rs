use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::detect::{NodejsTest, nodejs_test};

pub struct PkgJsToolsTestIsEmpty;

declare_rule! {
    /// The pkg-js-tools build test file has no commands.
    ///
    /// Nodejs is in use when `Build-Depends`, `Build-Depends-Indep`, or
    /// `Build-Depends-Arch` names `dh-sequence-nodejs`, or `debian/rules`
    /// passes `--with nodejs` to `dh`. A `dh` line that interpolates `$(…)`
    /// or `${…}` suppresses the Tag. The first existing file of
    /// `debian/nodejs/test` and `debian/tests/pkg-js/test` is empty when no
    /// line has a word character before a `#`. Extra is empty. The Location
    /// is that file. Applies to the Source tree and Source package.
    PkgJsToolsTestIsEmpty,
    code = "LN0403",
    tag = "pkg-js-tools-test-is-empty",
    default_selected = true,
    experimental = false,
}

impl SourceRule for PkgJsToolsTestIsEmpty {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(NodejsTest::Present { path, empty: true }) = nodejs_test(ctx) else {
            return;
        };
        ctx.diagnostic(Diagnostic::warn("").with_location(Location {
            path,
            line: None,
            column: None,
        }));
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "\
Source: example\n\
Maintainer: Example <ex@example.com>\n\
Build-Depends: debhelper-compat (= 13)\n\
\n\
Package: example\n\
Architecture: all\n\
Description: example\n extra\n";

    const WITH_SEQUENCE: &str = "\
Source: example\n\
Maintainer: Example <ex@example.com>\n\
Build-Depends: debhelper-compat (= 13), dh-sequence-nodejs\n\
\n\
Package: example\n\
Architecture: all\n\
Description: example\n extra\n";

    fn rules(body: &str) -> String {
        format!("#!/usr/bin/make -f\n\n%:\n\t{body}\n")
    }

    fn tree(control: &str, rules_body: &str, test: Option<&str>) -> Vec<(String, TestFile)> {
        let mut files = vec![
            ("debian/control".to_string(), TestFile::new(control)),
            ("debian/rules".to_string(), TestFile::new(rules(rules_body))),
        ];
        if let Some(body) = test {
            files.push(("debian/tests/pkg-js/test".to_string(), TestFile::new(body)));
        }
        files
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README".to_string(), TestFile::new("no packaging"))],
            tree(CONTROL, "dh $@ --with nodejs", Some("echo ok\n")),
            tree(CONTROL, "dh $@", Some("# Does nothing\n")),
            tree(
                CONTROL,
                "dh $(shell echo $@) --with nodejs",
                Some("# empty\n"),
            ),
            tree(WITH_SEQUENCE, "dh $@", None),
        ];
        let fail = vec![
            tree(CONTROL, "dh $@ --with nodejs", Some("# Does nothing\n")),
            tree(CONTROL, "dh $@ --with=nodejs", Some("\n")),
            tree(WITH_SEQUENCE, "dh $@", Some("# comment only\n\n")),
        ];
        SourceTreeTester::new(PkgJsToolsTestIsEmpty, pass, fail).test_and_snapshot();
    }
}
