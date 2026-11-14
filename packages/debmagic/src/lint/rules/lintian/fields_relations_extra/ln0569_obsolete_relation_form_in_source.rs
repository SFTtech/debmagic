use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::comma_list;

pub struct ObsoleteRelationFormInSource;

declare_rule! {
    /// A relation in `debian/control` uses the obsolete `<` or `>` operator.
    ///
    /// Source paragraph: `Build-Depends`, `Build-Depends-Indep`,
    /// `Build-Conflicts`, `Build-Conflicts-Indep`. Each installable:
    /// `Pre-Depends`, `Depends`, `Recommends`, `Suggests`, `Breaks`,
    /// `Conflicts`, `Provides`, `Replaces`, `Enhances`. Items are the
    /// comma-separated entries, each checked for `(< version)` or
    /// `(> version)`. Extra is `(in source paragraph)` or
    /// `(in section for <installable>)`, the field, and the item as written.
    ObsoleteRelationFormInSource,
    code = "LN0569",
    tag = "obsolete-relation-form-in-source",
    default_selected = true,
    experimental = false,
}

const SOURCE_FIELDS: &[&str] = &[
    "Build-Depends",
    "Build-Depends-Indep",
    "Build-Conflicts",
    "Build-Conflicts-Indep",
];

const INSTALLABLE_FIELDS: &[&str] = &[
    "Pre-Depends",
    "Depends",
    "Recommends",
    "Suggests",
    "Breaks",
    "Conflicts",
    "Provides",
    "Replaces",
    "Enhances",
];

static OBSOLETE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[(][<>]\s*[^<>=]+[)]").expect("valid regex"));

impl SourceRule for ObsoleteRelationFormInSource {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let mut messages = Vec::new();
        if let Some(control) = ctx.debian_control() {
            messages.extend(scan(
                control.source(),
                SOURCE_FIELDS,
                "(in source paragraph)",
            ));
            for installable in control.installables() {
                messages.extend(scan(
                    installable.fields(),
                    INSTALLABLE_FIELDS,
                    &format!("(in section for {})", installable.name()),
                ));
            }
        }
        for message in messages {
            ctx.diagnostic(Diagnostic::error(message));
        }
    }
}

fn scan(fields: &ControlParagraph, names: &[&str], section: &str) -> Vec<String> {
    names
        .iter()
        .flat_map(|field| {
            comma_list(fields, field)
                .into_iter()
                .filter(|item| OBSOLETE.is_match(item))
                .map(|item| format!("{section} {field} {item}"))
                .collect::<Vec<_>>()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::super::testing::raw;
    use super::*;

    #[test]
    fn test() {
        let pass = vec![
            Vec::<(&str, TestFile)>::new(),
            raw(
                "Source: example\nBuild-Depends: perl (>> 5.8), foo (<< 2), bar (<= 1), baz (>= 1)\n\n\
                 Package: example\nDepends: a (= 1), b (<<1), c\n",
            ),
            // Build-Depends-Arch is not a field this Check reads.
            raw("Source: example\nBuild-Depends-Arch: perl (> 5.8)\n"),
        ];
        let fail = vec![raw(
            "Source: example\nBuild-Depends: debhelper,\n perl (> 5.8), foo\n\
                 Build-Conflicts-Indep: bad (<1.0)\n\n\
                 Package: example\nDepends: a, b (< 2)\nBreaks: libsqlite3-0 (< 3.6.12)\n\n\
                 Package: example-two\nSuggests: c (> 1)\n",
        )];
        SourceTreeTester::new(ObsoleteRelationFormInSource, pass, fail).test_and_snapshot();
    }
}
