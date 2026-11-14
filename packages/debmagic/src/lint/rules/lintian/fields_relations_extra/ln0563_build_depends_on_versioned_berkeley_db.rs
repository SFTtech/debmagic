use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::{SOURCE_DEPENDENCY_FIELDS, alternatives_of};

pub struct BuildDependsOnVersionedBerkeleyDb;

declare_rule! {
    /// A build dependency names a versioned Berkeley DB development package.
    ///
    /// `Build-Depends`, `Build-Depends-Indep`, and `Build-Depends-Arch`. The
    /// package matches `libdb<N>.<N>...-dev`. Extra is the field and the
    /// package with no space between them (`Build-Depends:libdb5.1-dev`).
    BuildDependsOnVersionedBerkeleyDb,
    code = "LN0563",
    tag = "build-depends-on-versioned-berkeley-db",
    default_selected = true,
    experimental = false,
}

static VERSIONED_DB: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^libdb\d+\.\d+.*-dev$").expect("valid regex"));

impl SourceRule for BuildDependsOnVersionedBerkeleyDb {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let messages: Vec<String> = ctx
            .debian_control()
            .map(|control| {
                alternatives_of(control.source(), SOURCE_DEPENDENCY_FIELDS)
                    .into_iter()
                    .filter(|(_, alternative)| VERSIONED_DB.is_match(alternative.package()))
                    .map(|(field, alternative)| format!("{field}:{}", alternative.package()))
                    .collect()
            })
            .unwrap_or_default();
        for message in messages {
            ctx.diagnostic(Diagnostic::warn(message));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::super::testing::control;
    use super::*;

    #[test]
    fn test() {
        let pass = vec![
            Vec::<(&str, TestFile)>::new(),
            control(&[(
                "Build-Depends",
                "libdb-dev, libdb5-dev, libdb5.1, libdb5.1-doc",
            )]),
            control(&[("Build-Conflicts", "libdb5.1-dev")]),
        ];
        let fail = vec![
            control(&[("Build-Depends", "libdb5.1-java-dev, libdb5.1++-dev (>= 5)")]),
            control(&[
                ("Build-Depends-Indep", "foo | libdb4.8-dev"),
                ("Build-Depends-Arch", "libdb6.0-dev:native"),
            ]),
        ];
        SourceTreeTester::new(BuildDependsOnVersionedBerkeleyDb, pass, fail).test_and_snapshot();
    }
}
