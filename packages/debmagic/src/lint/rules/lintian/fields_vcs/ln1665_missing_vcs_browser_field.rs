use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::data::lines;
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Severity, SourceRule};

use super::super::vcs::vcs_field_value;

pub struct MissingVcsBrowserField;

declare_rule! {
    /// A `Vcs-*` field points at a hosting service with a web browser view,
    /// but the package declares no `Vcs-Browser`.
    ///
    /// Each hoster pattern of lintian's `fields/vcs-hosters` table is matched
    /// against the unfolded value of `Vcs-<platform>`, where `<platform>` is
    /// the table's first name for the hoster (`Git`, for example), at the
    /// start and case-insensitively. Reads the Dsc, or the source paragraph of
    /// `debian/control` when there is no Dsc, where an `XS-` style field
    /// counts under its plain name as on the Dsc. Extra is the field name and
    /// its value, for example `Vcs-Git https://salsa.debian.org/project/repo.git`.
    /// Location is empty. Severity is info and the Tag is not selected by
    /// default, as in lintian.
    MissingVcsBrowserField,
    code = "LN1665",
    tag = "missing-vcs-browser-field",
    default_selected = false,
    experimental = false,
}

/// The hoster table: each pattern with the platform it reports under.
static HOSTERS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    lines(include_str!("../../../data/fields/vcs-hosters"))
        .filter_map(|line| {
            let (pattern, platforms) = line.split_once("~~")?;
            let platform = platforms.split(',').next()?.trim();
            let regex = Regex::new(&format!("(?i)^(?:{})", pattern.trim())).ok()?;
            Some((regex, platform))
        })
        .collect()
});

impl SourceRule for MissingVcsBrowserField {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let source_tree = ctx.dsc().is_none();
        let messages = {
            let fields: &ControlParagraph = match ctx.dsc() {
                Some(dsc) => dsc.fields(),
                None => match ctx.debian_control() {
                    Some(control) => control.source(),
                    None => return,
                },
            };
            // An empty `Vcs-Browser` is not declared, as for LN1660.
            if vcs_field_value(fields, "Vcs-Browser", source_tree)
                .is_some_and(|value| !value.trim().is_empty())
            {
                return;
            }
            HOSTERS
                .iter()
                .filter_map(|(pattern, platform)| {
                    let name = format!("Vcs-{platform}");
                    let url = vcs_field_value(fields, &name, source_tree)?;
                    pattern.is_match(&url).then(|| format!("{name} {url}"))
                })
                .collect::<Vec<String>>()
        };
        for message in messages {
            ctx.diagnostic(Diagnostic::warn(message).with_severity(Severity::Info));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn control(fields: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: example\nMaintainer: Jane <jane@example.com>\n\
                 Standards-Version: 4.7.2\n{fields}\n\
                 Package: example\nArchitecture: all\nDescription: example\n extra\n"
            )),
        )]
    }

    #[test]
    fn test() {
        let pass = vec![
            control(""),
            control(
                "Vcs-Git: https://salsa.debian.org/project/repo.git\nVcs-Browser: https://salsa.debian.org/project/repo\n",
            ),
            control("Vcs-Git: https://example.com/project/repo.git\n"),
            control("Vcs-Svn: svn://svn.example.com/trunk\n"),
        ];
        let fail = vec![
            control("Vcs-Git: https://salsa.debian.org/project/repo.git\n"),
            control("Vcs-Git: https://GitHub.com/project/repo.git\n"),
            control("XS-Vcs-Git: https://salsa.debian.org/project/repo.git\n"),
        ];
        SourceTreeTester::new(MissingVcsBrowserField, pass, fail).test_and_snapshot();
    }
}
