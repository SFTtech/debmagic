use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::version_substvars::{self, Binnmu};

pub struct NotBinnmuableAnyDependsAll;

declare_rule! {
    /// An `Architecture: any` package depends on an `all` package with
    /// `${binary:Version}` or `${Source-Version}`.
    ///
    /// `Pre-Depends` and `Depends`. The operator is `=` or `>=`.
    /// `${source:Version}` is not this Tag. Extra is `<any> -> <all>`.
    NotBinnmuableAnyDependsAll,
    code = "LN0111",
    tag = "not-binnmuable-any-depends-all",
    default_selected = true,
    experimental = false,
}

impl SourceRule for NotBinnmuableAnyDependsAll {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let messages: Vec<String> = version_substvars::hits(control)
            .into_iter()
            .filter(|hit| hit.kind == Binnmu::AnyDependsAll)
            .map(|hit| hit.message)
            .collect();
        for message in messages {
            ctx.diagnostic(Diagnostic::error(message));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn tree(body: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: example\nMaintainer: Jane <jane@example.com>\n\
                 Standards-Version: 4.7.2\n\n{body}"
            )),
        )]
    }

    #[test]
    fn test() {
        let pass = vec![tree(
            "Package: program-bin\nArchitecture: any\n\
             Depends: program-data (= ${source:Version})\nDescription: bin\n extra\n\n\
             Package: program-data\nArchitecture: all\nDescription: data\n extra\n",
        )];
        let fail = vec![tree(
            "Package: program-bin\nArchitecture: any\n\
             Depends: program-data (= ${binary:Version})\nDescription: bin\n extra\n\n\
             Package: program-utils\nArchitecture: any\n\
             Depends: program-data (>= ${binary:Version})\nDescription: utils\n extra\n\n\
             Package: program-data\nArchitecture: all\nDescription: data\n extra\n",
        )];
        SourceTreeTester::new(NotBinnmuableAnyDependsAll, pass, fail).test_and_snapshot();
    }
}
