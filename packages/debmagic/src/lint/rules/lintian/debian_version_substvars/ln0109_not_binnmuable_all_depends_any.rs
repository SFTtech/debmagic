use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::version_substvars::{self, Binnmu};

pub struct NotBinnmuableAllDependsAny;

declare_rule! {
    /// An `Architecture: all` package depends on an `any` package with `= ${…Version}`.
    ///
    /// `Pre-Depends` and `Depends` in `debian/control`. The substvar is
    /// `source:Version`, `Source-Version`, or `binary:Version`, and the
    /// operator is `=`. Extra is `<all> -> <any>`.
    NotBinnmuableAllDependsAny,
    code = "LN0109",
    tag = "not-binnmuable-all-depends-any",
    default_selected = true,
    experimental = false,
}

impl SourceRule for NotBinnmuableAllDependsAny {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let messages: Vec<String> = version_substvars::hits(control)
            .into_iter()
            .filter(|hit| hit.kind == Binnmu::AllDependsAny)
            .map(|hit| hit.message)
            .collect();
        for message in messages {
            ctx.diagnostic(Diagnostic::error(message));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn control(body: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: example\nMaintainer: Jane <jane@example.com>\n\
                 Standards-Version: 4.7.2\n\n{body}"
            )),
        )]
    }

    fn tree(body: &str) -> Vec<(&'static str, TestFile)> {
        control(body)
    }

    #[test]
    fn test() {
        let pass = vec![tree(
            "Package: program-data\nArchitecture: all\n\
             Depends: program-bin (>= ${source:Version})\nDescription: data\n extra\n\n\
             Package: program-bin\nArchitecture: any\nDescription: bin\n extra\n",
        )];
        let fail = vec![tree(
            "Package: program-data-extra\nArchitecture: all\n\
             Depends: program-bin (= ${binary:Version})\nDescription: data\n extra\n\n\
             Package: program-bin\nArchitecture: any\nDescription: bin\n extra\n",
        )];
        SourceTreeTester::new(NotBinnmuableAllDependsAny, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree(
            "Package: program-data\nArchitecture: all\nDescription: data\n extra\n",
        ))];
        let fail = vec![SourcePackageCase::from(tree(
            "Package: program-data-extra\nArchitecture: all\n\
             Depends: program-bin (= ${source:Version})\nDescription: data\n extra\n\n\
             Package: program-bin\nArchitecture: any\nDescription: bin\n extra\n",
        ))];
        SourcePackageTester::new(NotBinnmuableAllDependsAny, pass, fail).test_and_snapshot();
    }
}
