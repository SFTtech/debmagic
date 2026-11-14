use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::super::fields_relations_more::SOURCE_RELATION_FIELDS;
use super::alternatives_of;

pub struct ConflictingNegationInSourceRelation;

declare_rule! {
    /// A source relation mixes negated and plain architectures in one `[...]`.
    ///
    /// Build-dependency and build-conflict fields of the source paragraph. An
    /// alternative counts when its architecture restriction has at least one
    /// `!` name and the number of negated names differs from the number of
    /// names. Extra is the field and the alternative as written.
    ConflictingNegationInSourceRelation,
    code = "LN0562",
    tag = "conflicting-negation-in-source-relation",
    default_selected = true,
    experimental = false,
}

impl SourceRule for ConflictingNegationInSourceRelation {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let messages: Vec<String> = ctx
            .debian_control()
            .map(|control| {
                alternatives_of(control.source(), SOURCE_RELATION_FIELDS)
                    .into_iter()
                    .filter(|(_, alternative)| {
                        alternative.negated_architectures() != 0
                            && alternative.negated_architectures()
                                != alternative.architectures().len()
                    })
                    .map(|(field, alternative)| format!("{field}: {}", alternative.text()))
                    .collect()
            })
            .unwrap_or_default();
        for message in messages {
            ctx.diagnostic(Diagnostic::error(message));
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
            control(&[("Build-Depends", "foo [amd64 i386], bar [!amd64 !i386], baz")]),
            control(&[("Build-Depends", "foo [!amd64]")]),
        ];
        let fail = vec![
            control(&[(
                "Build-Depends",
                "baz [i386 !amd64], foo | quux [!i386 amd64 arm64]",
            )]),
            control(&[
                ("Build-Depends-Arch", "gcc (>= 4) [amd64 !i386]"),
                ("Build-Conflicts", "bad [!amd64 i386]"),
            ]),
        ];
        SourceTreeTester::new(ConflictingNegationInSourceRelation, pass, fail).test_and_snapshot();
    }
}
