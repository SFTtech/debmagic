//! Rules for lintian's `fields/built-using` and `fields/static-built-using`
//! Checks. Each Rule owns one Tag. The two fields share the clause check.

use std::sync::LazyLock;

use regex::Regex;

use crate::lint::package_relation::{Alternative, PackageRelation};

mod ln0833_invalid_value_in_built_using_field;

pub(crate) use ln0833_invalid_value_in_built_using_field::InvalidValueInBuiltUsingField;

static PACKAGE_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z0-9][-+.a-z0-9]+$").expect("valid regex"));
static PACKAGE_VERSION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:\d+:)?[0-9][0-9A-Za-z.+:~]*(?:-[0-9A-Za-z.+:~]+)*$").expect("valid regex")
});

/// The first clause of a relation field that is not `source (= version)`,
/// as lintian prints it. An OR clause is never valid, so it is printed whole.
pub(crate) fn first_invalid_clause(relation: &PackageRelation) -> Option<String> {
    relation
        .dependencies()
        .iter()
        .find_map(|alternatives| match alternatives.as_slice() {
            [] => None,
            [alternative] => (!is_source_clause(alternative)).then(|| display(alternative)),
            several => Some(several.iter().map(display).collect::<Vec<_>>().join(" | ")),
        })
}

fn is_source_clause(alternative: &Alternative) -> bool {
    alternative.architecture_qualifier().is_empty()
        && alternative.architectures().is_empty()
        && alternative.profiles().is_empty()
        && alternative.rest().is_empty()
        && PACKAGE_NAME.is_match(alternative.package())
        && alternative.version().is_some_and(|constraint| {
            constraint.operator == "=" && PACKAGE_VERSION.is_match(&constraint.version)
        })
}

/// The alternative normalised as lintian's `Relation` prints it.
fn display(alternative: &Alternative) -> String {
    if !alternative.rest().is_empty() {
        return alternative.text().to_string();
    }
    let mut text = alternative.package().to_string();
    if !alternative.architecture_qualifier().is_empty() {
        text.push(':');
        text.push_str(alternative.architecture_qualifier());
    }
    if let Some(constraint) = alternative.version() {
        text.push_str(&format!(
            " ({} {})",
            constraint.operator, constraint.version
        ));
    }
    if !alternative.architectures().is_empty() {
        text.push_str(&format!(" [{}]", alternative.architectures().join(" ")));
    }
    for profile in alternative.profiles() {
        text.push_str(&format!(" <{}>", profile.join(" ")));
    }
    text
}
