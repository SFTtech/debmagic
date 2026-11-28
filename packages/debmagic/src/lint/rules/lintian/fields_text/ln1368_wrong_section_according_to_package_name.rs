use std::sync::LazyLock;

use regex::bytes::{Regex as BytesRegex, RegexBuilder as BytesRegexBuilder};
use regex::{Regex, RegexBuilder};

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::data;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, Severity};

pub struct WrongSectionAccordingToPackageName;

declare_rule! {
    /// The `Section` does not match the section the package name suggests, such
    /// as `devel` for `python3-foo` (which belongs to `python`).
    ///
    /// The vendored `fields/name_section_mappings` patterns are read in file
    /// order, with lintian's `x` flag (whitespace in a pattern is ignored). The
    /// first pattern that matches the package name decides: when the Section's
    /// last part differs from its section, the Rule reports `<section> =>
    /// <better>`, where `<better>` keeps the archive area (`contrib/`) of the
    /// Section. A `debug` Section on a package not named `-dbg` or `-dbgsym` is
    /// reported with extra the Section as written. Sections of `oldlibs` skip
    /// the name mappings. Location is empty. Udebs are skipped.
    WrongSectionAccordingToPackageName,
    code = "LN1368",
    tag = "wrong-section-according-to-package-name",
    default_selected = false,
    experimental = false,
}

/// The table in Unicode mode, for a package name that is not ASCII.
static MAPPINGS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    data::lines(include_str!("../../../data/fields/name_section_mappings"))
        .filter_map(|line| {
            let (pattern, section) = line.split_once("=>")?;
            let regex = RegexBuilder::new(pattern.trim())
                .ignore_whitespace(true)
                .build()
                .ok()?;
            Some((regex, section.trim()))
        })
        .collect()
});

/// The table in ASCII mode, for an ASCII package name. On ASCII text the
/// patterns match the same as in Unicode mode, and ASCII classes are much
/// cheaper to compile. The bytes API takes `.` and `\S` without the `&str`
/// restriction on partial characters.
static ASCII_MAPPINGS: LazyLock<Vec<(BytesRegex, &'static str)>> = LazyLock::new(|| {
    data::lines(include_str!("../../../data/fields/name_section_mappings"))
        .filter_map(|line| {
            let (pattern, section) = line.split_once("=>")?;
            let regex = BytesRegexBuilder::new(pattern.trim())
                .ignore_whitespace(true)
                .unicode(false)
                .build()
                .ok()?;
            Some((regex, section.trim()))
        })
        .collect()
});

/// The section of the first table pattern that matches `package`.
fn mapped_section(package: &str) -> Option<&'static str> {
    if package.is_ascii() {
        ASCII_MAPPINGS
            .iter()
            .find(|(pattern, _)| pattern.is_match(package.as_bytes()))
            .map(|(_, section)| *section)
    } else {
        MAPPINGS
            .iter()
            .find(|(pattern, _)| pattern.is_match(package))
            .map(|(_, section)| *section)
    }
}

static DEBUG_PACKAGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"-dbg(?:sym)?$").expect("valid regex"));

impl BinaryPackageRule for WrongSectionAccordingToPackageName {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let fields = ctx.installation_control().fields();
        let Some(section) = fields.unfolded_value("Section") else {
            return;
        };
        let (division, fraction) = match section.split_once('/') {
            Some((division, fraction)) => (Some(division), fraction),
            None => (None, section.as_str()),
        };
        let package = ctx.package_name().to_string();

        let mut messages = Vec::new();
        if fraction != "oldlibs"
            && let Some(want) = mapped_section(&package)
            && fraction != want
        {
            let better = match division {
                Some(division) => format!("{division}/{want}"),
                None => want.to_string(),
            };
            messages.push(format!("{section} => {better}"));
        }
        if fraction == "debug" && !DEBUG_PACKAGE.is_match(&package) {
            messages.push(section.clone());
        }
        for message in messages {
            ctx.diagnostic(Diagnostic::warn(message).with_severity(Severity::Info));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    /// The ASCII table keeps every mapping of the Unicode table, in order.
    #[test]
    fn ascii_table_has_every_mapping() {
        assert_eq!(super::ASCII_MAPPINGS.len(), super::MAPPINGS.len());
    }

    use super::*;

    fn section(name: &str, section: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .package(name)
            .field("Section", section)
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            section("python3-foo", "python"),
            section("libfoo-dev", "libdevel"),
            section("foo", "devel"),
            section("libfoo-dbg", "debug"),
            section("foo-dbgsym", "debug"),
            // Oldlibs skips the name mappings.
            section("libfoo-perl", "oldlibs"),
            section("contrib-doc-foo", "contrib/doc"),
            // Udebs are not checked.
            section("libfoo", "libs").udeb(),
        ];
        let fail = vec![
            section("python3-foo", "devel"),
            section("libfoo-perl", "devel"),
            section("foo", "debug"),
            section("foo-docs", "contrib/devel"),
        ];
        BinaryPackageTester::new(WrongSectionAccordingToPackageName, pass, fail)
            .test_and_snapshot();
    }
}
