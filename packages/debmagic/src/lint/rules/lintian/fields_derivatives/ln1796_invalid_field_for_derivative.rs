use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_control::ControlParagraph;
use crate::lint::derivative::{self, FieldPattern};
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

pub struct InvalidFieldForDerivative;

declare_rule! {
    /// A field of the source paragraph does not match the derivative pattern of
    /// lintian's `data/fields/derivative-fields`. The extra is the field, its
    /// value and the explanation, as `Field: value (explanation)`; a missing or
    /// empty value is shown as `-`, and one hint for each entry it breaks.
    ///
    /// Debian's table has one entry, `Source ~~ . ~~ always matches`. The value
    /// shown as `-` when it is missing or empty matches `.`, and so does every
    /// other non-empty value, so Debian never gets this Tag: the Rule reports
    /// nothing. The Rule reads the Dsc, or on a Source tree the source paragraph
    /// of `debian/control`. Lintian's `value` is trimmed, and so is this one.
    InvalidFieldForDerivative,
    code = "LN1796",
    tag = "invalid-field-for-derivative",
    default_selected = true,
    experimental = false,
}

impl SourceRule for InvalidFieldForDerivative {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(fields) = derivative::source_fields(ctx) else {
            return;
        };
        let reports = invalid_fields(fields, derivative::fields());
        for extra in reports {
            ctx.diagnostic(Diagnostic::error(extra));
        }
    }
}

/// One `Field: value (explanation)` for each entry whose pattern the field's
/// value does not match, in table order. Lintian takes `value || '-'`, so an
/// empty value and the value `0` are checked as `-`.
fn invalid_fields(fields: &ControlParagraph, table: &[FieldPattern]) -> Vec<String> {
    table
        .iter()
        .filter_map(|entry| {
            let value = match fields.get(&entry.field).map(str::trim) {
                Some(value) if !value.is_empty() && value != "0" => value,
                _ => "-",
            };
            (!entry.pattern.regex.is_match(value))
                .then(|| format!("{}: {value} ({})", entry.field, entry.pattern.explanation))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::derivative::Pattern;
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn tree(control: &str) -> Vec<(&'static str, TestFile)> {
        vec![("debian/control", TestFile::new(control.to_string()))]
    }

    fn dsc(text: &str) -> Dsc {
        Dsc::parse(text).expect("Dsc fixture")
    }

    fn table(field: &str, regex: &str, explanation: &str) -> Vec<FieldPattern> {
        vec![FieldPattern {
            field: field.to_string(),
            pattern: Pattern {
                regex: regex::Regex::new(regex).expect("test regex"),
                explanation: explanation.to_string(),
            },
        }]
    }

    #[test]
    fn source_tree() {
        // Debian's table matches every value, so nothing is reported.
        let pass = vec![
            Vec::<(&str, TestFile)>::new(),
            tree("Source: example\n"),
            tree("Source:\n"),
            tree("Source: example\nVcs-Git: https://example.com/x.git\n"),
        ];
        let fail = Vec::<Vec<(&str, TestFile)>>::new();
        SourceTreeTester::new(InvalidFieldForDerivative, pass, fail).test();
    }

    #[test]
    fn source_package() {
        let pass = vec![
            SourcePackageCase::from(Vec::<(&str, TestFile)>::new()).dsc(dsc(
                "Format: 3.0 (quilt)\nSource: example\nVersion: 1.0-1\n",
            )),
            SourcePackageCase::from(Vec::<(&str, TestFile)>::new())
                .dsc(dsc("Format: 3.0 (quilt)\nVersion: 1.0-1\n")),
        ];
        let fail = Vec::<SourcePackageCase>::new();
        SourcePackageTester::new(InvalidFieldForDerivative, pass, fail).test();
    }

    #[test]
    fn reports_each_entry_a_value_breaks() {
        let control = ControlParagraph::from_fields([("Source", "lower")]);
        assert_eq!(
            invalid_fields(&control, &table("Source", "^[a-z]+$", "lower case")),
            Vec::<String>::new()
        );
        assert_eq!(
            invalid_fields(&control, &table("Source", "^[A-Z]+$", "upper case")),
            vec!["Source: lower (upper case)".to_string()]
        );
    }

    #[test]
    fn missing_and_zero_values_are_checked_as_a_hyphen() {
        let empty = ControlParagraph::from_fields([("Source", "")]);
        let zero = ControlParagraph::from_fields([("Source", "0")]);
        let missing = ControlParagraph::from_fields([("Other", "x")]);
        // `-` matches the pattern, so no value of the field is reported.
        let hyphen = table("Source", "^-$", "hyphen only");
        assert_eq!(invalid_fields(&empty, &hyphen), Vec::<String>::new());
        assert_eq!(invalid_fields(&zero, &hyphen), Vec::<String>::new());
        assert_eq!(invalid_fields(&missing, &hyphen), Vec::<String>::new());
        // `-` does not match, so a missing field is reported as `-`.
        let letters = table("Source", "^[a-z]+$", "needs a value");
        assert_eq!(
            invalid_fields(&missing, &letters),
            vec!["Source: - (needs a value)".to_string()]
        );
    }
}
