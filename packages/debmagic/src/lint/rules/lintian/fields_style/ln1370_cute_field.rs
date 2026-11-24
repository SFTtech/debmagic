use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

pub struct CuteField;

declare_rule! {
    /// A field name in `debian/control` is not in the usual capitalization,
    /// such as `Rules-requires-root` for `Rules-Requires-Root`.
    ///
    /// Every field name of the source paragraph and of each installable
    /// paragraph is compared with its title-cased form: each word starts with a
    /// capital, and an `X` prefix followed by up to three of `S`, `B`, `C` and
    /// a hyphen is all capitals. `X-DH-Compat` is the one exception. Extra is
    /// `<name> vs <standard>`. Location is `debian/control` with no line. Pedantic.
    /// Applies to the Source tree and Source package.
    CuteField,
    code = "LN1370",
    tag = "cute-field",
    default_selected = false,
    experimental = false,
}

/// The name as lintian's `fields/style` Check expects it.
fn standard_name(name: &str) -> String {
    let lower = name.to_lowercase();
    if lower == "x-dh-compat" {
        return "X-DH-Compat".to_string();
    }
    let mut title = String::with_capacity(lower.len());
    let mut previous_word = false;
    for c in lower.chars() {
        let word = c.is_alphanumeric() || c == '_';
        if word && !previous_word {
            title.extend(c.to_uppercase());
        } else {
            title.push(c);
        }
        previous_word = word;
    }
    capitalize_x_prefix(title)
}

/// `^(X[SBC]{1,3})-` case-insensitively, with the matched part in capitals.
fn capitalize_x_prefix(name: String) -> String {
    let chars: Vec<char> = name.chars().collect();
    let letters = chars
        .iter()
        .skip(1)
        .take_while(|c| matches!(c.to_ascii_uppercase(), 'S' | 'B' | 'C'))
        .count();
    let x = matches!(chars.first(), Some('x' | 'X'));
    if x && (1..=3).contains(&letters) && chars.get(1 + letters) == Some(&'-') {
        chars[..=letters]
            .iter()
            .flat_map(|c| c.to_uppercase())
            .chain(chars[letters + 1..].iter().copied())
            .collect()
    } else {
        name
    }
}

/// Each mismatch with the line of its field name (lintian's pointer).
fn mismatches(fields: &ControlParagraph) -> Vec<(String, Option<u32>)> {
    fields
        .names()
        .filter_map(|name| {
            let standard = standard_name(name);
            (name != standard).then(|| (format!("{name} vs {standard}"), fields.position(name)))
        })
        .collect()
}

impl SourceRule for CuteField {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let mut messages = mismatches(control.source());
        for installable in control.installables() {
            messages.extend(mismatches(installable.fields()));
        }
        for (message, line) in messages {
            ctx.diagnostic(
                Diagnostic::warn(message)
                    .with_severity(Severity::Pedantic)
                    .with_location(Location {
                        path: PathBuf::from("debian/control"),
                        line,
                        column: None,
                    }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn control(source_fields: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: example\nMaintainer: Example <ex@example.com>\n{source_fields}\nPackage: example\nArchitecture: all\nDescription: example\n extra\n"
            )),
        )]
    }

    #[test]
    fn standard_names() {
        assert_eq!(standard_name("Rules-Requires-Root"), "Rules-Requires-Root");
        assert_eq!(standard_name("rules-requires-root"), "Rules-Requires-Root");
        assert_eq!(standard_name("HOMEPAGE"), "Homepage");
        assert_eq!(standard_name("xs-python-version"), "XS-Python-Version");
        assert_eq!(standard_name("x-dh-compat"), "X-DH-Compat");
        assert_eq!(standard_name("X-Dh-Compat"), "X-DH-Compat");
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no control"))],
            control("Homepage: https://example.org/\n"),
            control("Rules-Requires-Root: no\nX-DH-Compat: 13\n"),
        ];
        let fail = vec![
            control("HOMEPAGE: https://example.org/\n"),
            control("Rules-requires-root: no\n"),
            control("X-Dh-Compat: 13\n"),
        ];
        SourceTreeTester::new(CuteField, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(control(
            "Homepage: https://example.org/\n",
        ))];
        let fail = vec![SourcePackageCase::from(control("Xs-Python-Version: 3.0\n"))];
        SourcePackageTester::new(CuteField, pass, fail).test_and_snapshot();
    }

    /// lintian's `fields/style` Check reports each mismatched name at its
    /// field's pointer (`$item->pointer($position)`), so the Location is the
    /// line of the field name. The `test()` snapshot pins the line numbers.
    #[test]
    fn mismatch_below_header_is_reported_at_its_line() {
        let pass = vec![control("Homepage: https://example.org/\n")];
        let fail = vec![control(
            "Homepage: https://example.org/\nRules-requires-root: no\n",
        )];
        SourceTreeTester::new(CuteField, pass, fail).test();
    }

    /// Boundary: a name that is already in the standard form, including the
    /// `XS-`/`XB-`/`XC-` prefixes, is not reported (`standard_name`).
    #[test]
    fn standard_prefixed_names_are_not_reported() {
        let pass = vec![control("XS-Python-Version: 3.0\nXB-Ruby-Versions: all\n")];
        let fail = vec![control("Xs-Python-Version: 3.0\n")];
        SourceTreeTester::new(CuteField, pass, fail).test();
    }
}
