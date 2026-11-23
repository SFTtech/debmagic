use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::description_field::description;

pub struct PerlModuleNameNotMentionedInDescription;

declare_rule! {
    /// A `lib<name>-perl` package ships a `.pm` file, but its extended
    /// description does not name the module.
    ///
    /// The module is `<name>` split at `-`, each part capitalized, joined with
    /// `::`: `libdeb-long-description-perl` names `Deb::Long::Description`.
    /// Reported when an installed file ends in `Deb/Long/Description.pm`, or in
    /// `Description.pm` (the last part), case-insensitively, and the lowercased
    /// extended description does not contain `deb::long::description`. The
    /// extended description is checked even when there is none. Extra is the
    /// module name. Location is empty. Severity is pedantic and the Tag is not
    /// selected by default, as in lintian. Only packages whose name matches
    /// `^lib(.+)-perl$` are checked. Lintian's Tag name is
    /// `no-module-name-in-description` in pkg-perl-tools.
    PerlModuleNameNotMentionedInDescription,
    code = "LN1669",
    tag = "perl-module-name-not-mentioned-in-description",
    default_selected = false,
    experimental = false,
}

static PERL_PACKAGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^lib(.+)-perl$").expect("valid regex"));

impl BinaryPackageRule for PerlModuleNameNotMentionedInDescription {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(captures) = PERL_PACKAGE.captures(ctx.package_name()) else {
            return;
        };
        let parts: Vec<String> = captures[1].split('-').map(capitalize).collect();
        let module = parts.join("::");
        let Some(last) = parts.last() else {
            return;
        };
        let path = format!("{}.pm", parts.join("/"));
        let file = format!("{last}.pm");
        let pm_found = ctx.data_files().is_some_and(|files| {
            files.entries().iter().any(|entry| {
                let name = entry.path().to_string_lossy().to_lowercase();
                name.ends_with(&path.to_lowercase())
                    || name.ends_with(&format!("/{}", file.to_lowercase()))
            })
        });
        if !pm_found {
            return;
        }
        let extended = description(ctx)
            .map(|description| description.lines.join("\n"))
            .unwrap_or_default()
            .to_lowercase();
        if extended.contains(&module.to_lowercase()) {
            return;
        }
        ctx.diagnostic(Diagnostic::warn(module).with_severity(Severity::Pedantic));
    }
}

/// Perl's `ucfirst` on one part of a package name.
fn capitalize(part: &str) -> String {
    let mut chars = part.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn perl(package: &str, description: &str, files: &[&str]) -> BinaryPackageCase {
        let mut case = BinaryPackageCase::from(
            files
                .iter()
                .map(|path| {
                    (
                        path.to_string(),
                        TestFile::new("package Deb::Long::Description;\n"),
                    )
                })
                .collect::<Vec<_>>(),
        )
        .package(package);
        if !description.is_empty() {
            case = case.field("Description", description);
        }
        case
    }

    const PM: &str = "usr/share/perl5/Deb/Long/Description.pm";

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            perl(
                "libdeb-long-description-perl",
                "A module.\n A Deb::Long::Description module.",
                &[PM],
            ),
            perl(
                "libdeb-long-description-perl",
                "A module.\n A deb::long::description module.",
                &[PM],
            ),
            // No module file, nothing to name.
            perl(
                "libdeb-long-description-perl",
                "tool\n A tool.",
                &["usr/share/doc/x"],
            ),
            // Not a Perl library package.
            perl("deb-long-description", "tool\n A tool.", &[PM]),
        ];
        let fail = vec![
            perl("libdeb-long-description-perl", "tool\n A module.", &[PM]),
            // The last part alone is enough to find the module file.
            perl(
                "libdeb-long-description-perl",
                "tool\n A module.",
                &["usr/share/perl5/Description.pm"],
            ),
            perl("libdeb-long-description-perl", "", &[PM]),
        ];
        BinaryPackageTester::new(PerlModuleNameNotMentionedInDescription, pass, fail)
            .test_and_snapshot();
    }
}
