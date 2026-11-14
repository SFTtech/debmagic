use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::version_substvars::{self, Binnmu};

pub struct NotBinnmuableAnyDependsAny;

declare_rule! {
    /// An `Architecture: any` package depends on another `any` package with
    /// `= ${source:Version}`.
    ///
    /// `Pre-Depends` and `Depends` in `debian/control`. `${binary:Version}`
    /// and `${Source-Version}` are not this Tag. Extra is `<from> -> <to>`.
    NotBinnmuableAnyDependsAny,
    code = "LN0110",
    tag = "not-binnmuable-any-depends-any",
    default_selected = true,
    experimental = false,
}

impl SourceRule for NotBinnmuableAnyDependsAny {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let messages: Vec<String> = version_substvars::hits(control)
            .into_iter()
            .filter(|hit| hit.kind == Binnmu::AnyDependsAny)
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
        let pass = vec![
            tree(
                "Package: program-utils\nArchitecture: any\n\
                 Depends: program-bin (>= ${source:Version})\nDescription: utils\n extra\n\n\
                 Package: program-bin\nArchitecture: any\nDescription: bin\n extra\n",
            ),
            tree(
                "Package: program-utils\nArchitecture: any\n\
                 Depends: program-bin (= ${binary:Version})\nDescription: utils\n extra\n\n\
                 Package: program-bin\nArchitecture: any\nDescription: bin\n extra\n",
            ),
        ];
        let fail = vec![tree(
            "Package: program-utils\nArchitecture: any\n\
             Depends: program-bin:any (= ${source:Version})\nDescription: utils\n extra\n\n\
             Package: program-bin\nArchitecture: any\nDescription: bin\n extra\n",
        )];
        SourceTreeTester::new(NotBinnmuableAnyDependsAny, pass, fail).test_and_snapshot();
    }
}
