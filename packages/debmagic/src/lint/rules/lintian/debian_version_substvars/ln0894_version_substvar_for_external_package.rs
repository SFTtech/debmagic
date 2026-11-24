use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_control::DebianControl;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct VersionSubstvarForExternalPackage;

declare_rule! {
    /// A relation on a package this source does not build, versioned with a
    /// `${source:…}` or `${binary:…}` version substitution variable.
    ///
    /// Checked for `Depends`, `Pre-Depends`, `Recommends`, `Suggests`,
    /// `Conflicts`, and `Replaces` of every installable in Debian control, in
    /// that order. A predicate `pkg[:qualifier] (op ${source:Version})` or
    /// `(op ${binary:Version})`, or the `Upstream-Version` forms, names
    /// `pkg`. The Tag fires for each distinct (field, substvar, pkg) unless
    /// `pkg` is an installable of this source (it declares `Architecture`),
    /// the installable's `Provides` lists `pkg (= ${substvar})` exactly, or
    /// `pkg` itself contains a `${…}` substitution. Extra is
    /// `<field> <substvar> <installable> -> <pkg>`. The Location is
    /// `debian/control` at the line of the field name, as lintian's pointer
    /// is, so an override such as `[debian/control:*]` matches it. Applies to the Source tree and Source
    /// package. Like lintian, only the `source:` and `binary:` forms count:
    /// the obsolete `${Source-Version}` is not matched here (lintian reports
    /// it as `substvar-source-version-is-deprecated`). The vendored
    /// `version-substvars-obsolete` hints predate lintian's current extra
    /// format; Parity corrects them to what the Perl prints.
    VersionSubstvarForExternalPackage,
    code = "LN0894",
    tag = "version-substvar-for-external-package",
    default_selected = true,
    experimental = false,
}

const FIELDS: [&str; 6] = [
    "Depends",
    "Pre-Depends",
    "Recommends",
    "Suggests",
    "Conflicts",
    "Replaces",
];

/// Lintian's `$PKGNAME_REGEX` anchored at the start, then the version relation
/// and the substvar it uses.
static VERSIONED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^([a-z0-9][-+.a-z0-9]+)(?::[-a-z0-9]+)?\s*\(\s*[><]?[=><]\s*(\$\{(?:source:|binary:)(?:Upstream-)?Version\})",
    )
    .expect("valid regex")
});

static SUBSTVAR_IN_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\$\{\S+\}").expect("valid regex"));

impl SourceRule for VersionSubstvarForExternalPackage {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let messages = match ctx.debian_control() {
            Some(control) => external_relations(control),
            None => return,
        };
        for (line, message) in messages {
            ctx.diagnostic(Diagnostic::error(message).with_location(Location {
                path: "debian/control".into(),
                line,
                column: None,
            }));
        }
    }
}

/// The extra of each external versioned relation, in lintian's order.
fn external_relations(control: &DebianControl) -> Vec<(Option<u32>, String)> {
    let provides: Vec<String> = control
        .installables()
        .iter()
        .flat_map(|installable| {
            installable
                .fields()
                .unfolded_value("Provides")
                .unwrap_or_default()
                .split(',')
                .map(|item| item.trim().to_string())
                .collect::<Vec<_>>()
        })
        .collect();

    let mut messages = Vec::new();
    for installable in control.installables() {
        let fields = installable.fields();
        for field in FIELDS {
            if !fields.declares(field) {
                continue;
            }
            let Some(relation) = fields.relation(field) else {
                continue;
            };
            // Ordered (substvar, package) pairs, first occurrence wins.
            let mut external: Vec<(String, String)> = Vec::new();
            for alternative in relation.alternatives() {
                let text = alternative.text();
                let Some(captures) = VERSIONED.captures(text) else {
                    continue;
                };
                let pair = (captures[2].to_string(), captures[1].to_string());
                if !external.contains(&pair) {
                    external.push(pair);
                }
            }
            for (substvar, other) in external {
                let declared_here = control
                    .installables()
                    .iter()
                    .find(|item| item.name() == other)
                    .is_some_and(|item| item.fields().declares("Architecture"));
                let provided = provides.contains(&format!("{other} (= {substvar})"));
                if declared_here || provided || SUBSTVAR_IN_NAME.is_match(&other) {
                    continue;
                }
                messages.push((
                    fields.position(field),
                    format!("{field} {substvar} {} -> {other}", installable.name()),
                ));
            }
        }
    }
    messages
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestCase, TestFile};

    use super::*;

    fn control(body: &str) -> TestCase {
        TestCase::source_tree([("debian/control", TestFile::new(body))])
    }

    const HEAD: &str = "\
Source: example
Maintainer: Maintainer <m@example.com>

";

    #[test]
    fn source() {
        let pass = vec![
            // Own package, with and without a qualifier.
            control(&format!(
                "{HEAD}Package: program-bin\nArchitecture: any\nDepends: program-data (= ${{binary:Version}})\n\nPackage: program-data\nArchitecture: all\nDepends: program-bin (= ${{source:Version}})\n"
            )),
            // Provides matches exactly.
            control(&format!(
                "{HEAD}Package: program-bin\nArchitecture: any\nProvides: foreign-pkg (= ${{binary:Version}})\n\nPackage: program-data\nArchitecture: all\nDepends: foreign-pkg (= ${{binary:Version}})\n"
            )),
            // The other name holds a substitution variable.
            control(&format!(
                "{HEAD}Package: program-data\nArchitecture: all\nDepends: foreign-${{suffix}} (= ${{source:Version}})\n"
            )),
            // No versioned substvar: a fixed version.
            control(&format!(
                "{HEAD}Package: program-data\nArchitecture: all\nDepends: foreign-pkg (= 1.0), foreign-two\n"
            )),
            // Not a version substvar at all (obsolete ${Source-Version}).
            control(&format!(
                "{HEAD}Package: program-data\nArchitecture: all\nDepends: foreign-pkg (= ${{Source-Version}})\n"
            )),
            // Field not in the checked list.
            control(&format!(
                "{HEAD}Package: program-data\nArchitecture: all\nBuilt-Using: foreign-pkg (= ${{source:Version}})\n"
            )),
        ];
        let fail = vec![
            control(&format!(
                "{HEAD}Package: program-data\nArchitecture: all\nDepends: foreign-pkg:any (= ${{source:Version}})\n"
            )),
            control(&format!(
                "{HEAD}Package: program-data\nArchitecture: all\nReplaces: other-foreign-pkg:any (<< ${{binary:Version}})\n"
            )),
            control(&format!(
                "{HEAD}Package: program-data\nArchitecture: all\nRecommends: foreign-pkg (>= ${{source:Upstream-Version}})\n"
            )),
            // Same package and substvar twice in one field is one Tag.
            control(&format!(
                "{HEAD}Package: program-data\nArchitecture: all\nSuggests: foreign-pkg (= ${{binary:Version}}), foreign-pkg (>= ${{binary:Version}})\n"
            )),
        ];
        SourceTreeTester::new(VersionSubstvarForExternalPackage, pass, fail).test_and_snapshot();
    }

    #[test]
    fn location_is_the_field_line() {
        // lintian's pointer is `debian/control:<line of the field>`, which
        // overrides such as `[debian/control:*]` match.
        let text = format!(
            "{HEAD}Package: program-data\nArchitecture: all\nDepends: foreign-pkg (= ${{source:Version}})\n"
        );
        let control = DebianControl::parse(&text).expect("parses");
        assert_eq!(
            external_relations(&control),
            vec![(
                Some(6),
                "Depends ${source:Version} program-data -> foreign-pkg".to_string()
            )]
        );
    }
}
