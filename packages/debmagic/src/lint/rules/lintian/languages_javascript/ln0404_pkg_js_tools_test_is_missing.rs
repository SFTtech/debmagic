use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::detect::{NodejsTest, nodejs_test};

pub struct PkgJsToolsTestIsMissing;

declare_rule! {
    /// pkg-js-tools would run a build test, and neither test file exists.
    ///
    /// Same nodejs detection as the empty-test Tag. The candidates are
    /// `debian/nodejs/test` then `debian/tests/pkg-js/test`. Extra is empty.
    /// There is no Location. Applies to the Source tree and Source package.
    PkgJsToolsTestIsMissing,
    code = "LN0404",
    tag = "pkg-js-tools-test-is-missing",
    default_selected = true,
    experimental = false,
}

impl SourceRule for PkgJsToolsTestIsMissing {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        if matches!(nodejs_test(ctx), Some(NodejsTest::Missing)) {
            ctx.diagnostic(Diagnostic::warn(""));
        }
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
            files.push(("debian/nodejs/test".to_string(), TestFile::new(body)));
        }
        files
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README".to_string(), TestFile::new("no packaging"))],
            tree(CONTROL, "dh $@", None),
            tree(CONTROL, "dh $@ --with nodejs", Some("echo ok\n")),
            tree(WITH_SEQUENCE, "dh $(shell echo $@)", None),
            vec![("debian/control".to_string(), TestFile::new(WITH_SEQUENCE))],
        ];
        let fail = vec![
            tree(CONTROL, "dh $@ --with nodejs", None),
            tree(CONTROL, "dh $@ --with nodejs,foo", None),
            tree(WITH_SEQUENCE, "dh $@", None),
        ];
        SourceTreeTester::new(PkgJsToolsTestIsMissing, pass, fail).test_and_snapshot();
    }
}
