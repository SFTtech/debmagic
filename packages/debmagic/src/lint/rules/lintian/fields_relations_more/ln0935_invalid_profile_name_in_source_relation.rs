use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::SOURCE_RELATION_FIELDS;

pub struct InvalidProfileNameInSourceRelation;

declare_rule! {
    /// A build profile in a restriction formula of a source relation is not registered.
    ///
    /// Each `<...>` profile of `Build-Depends`, `Build-Depends-Indep`,
    /// `Build-Depends-Arch`, and the matching `Build-Conflicts` fields, with a
    /// leading `!` removed, must be listed in Debian's registered build
    /// profiles (`fields/build-profiles`) or match `pkg.<source>.<name>`.
    /// Extra is the profile and `[field: alternative]`. Applies to source trees
    /// and source packages.
    InvalidProfileNameInSourceRelation,
    code = "LN0935",
    tag = "invalid-profile-name-in-source-relation",
    default_selected = true,
    experimental = false,
}

impl SourceRule for InvalidProfileNameInSourceRelation {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let messages = ctx
            .debian_control()
            .map(|control| messages(control.source()))
            .unwrap_or_default();
        for message in messages {
            ctx.diagnostic(Diagnostic::error(message));
        }
    }
}

static KNOWN_PROFILES: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    crate::lint::data::lines(include_str!("../../../data/fields/build-profiles")).collect()
});

static PKG_PROFILE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^pkg\.[a-z0-9][a-z0-9+.\-]+\..").expect("valid regex"));

fn messages(fields: &crate::lint::debian_control::ControlParagraph) -> Vec<String> {
    SOURCE_RELATION_FIELDS
        .iter()
        .filter_map(|field| Some((*field, fields.relation(field)?)))
        .flat_map(|(field, relation)| {
            relation
                .alternatives()
                .flat_map(|alternative| {
                    alternative
                        .profiles()
                        .iter()
                        .flatten()
                        .map(|profile| profile.strip_prefix('!').unwrap_or(profile.as_str()))
                        .filter(|profile| {
                            !KNOWN_PROFILES.contains(profile) && !PKG_PROFILE.is_match(profile)
                        })
                        .map(|profile| format!("{profile} [{field}: {}]", alternative.text()))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn control(fields: &[(&str, &str)]) -> Vec<(&'static str, TestFile)> {
        let mut body = String::from("Source: example\n");
        for (name, value) in fields {
            body.push_str(name);
            body.push_str(": ");
            body.push_str(value);
            body.push('\n');
        }
        body.push_str("\nPackage: example\nArchitecture: all\nDescription: example\n extra\n");
        vec![("debian/control", TestFile::new(body))]
    }

    #[test]
    fn test() {
        let pass = vec![
            control(&[]),
            control(&[(
                "Build-Depends",
                "foo <!nocheck>, bar <cross !nodoc> [amd64]",
            )]),
            control(&[("Build-Depends", "foo <pkg.example.thing>")]),
            control(&[("Build-Conflicts-Arch", "foo <!stage1>")]),
            // Only the source relation fields are walked.
            vec![(
                "debian/control",
                TestFile::new(
                    "Source: example\nBuild-Depends: debhelper\n\n\
                     Package: example\nArchitecture: all\nDepends: foo <foobar>\n\
                     Description: example\n extra\n",
                ),
            )],
        ];
        let fail = vec![
            control(&[("Build-Depends", "foobar <foobar>")]),
            control(&[
                ("Build-Depends", "foo <!nocheck nope>"),
                ("Build-Conflicts", "bar <nope>"),
            ]),
        ];
        SourceTreeTester::new(InvalidProfileNameInSourceRelation, pass, fail).test_and_snapshot();
    }
}
