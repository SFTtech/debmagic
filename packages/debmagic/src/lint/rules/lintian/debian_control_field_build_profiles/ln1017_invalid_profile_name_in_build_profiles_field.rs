use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct InvalidProfileNameInBuildProfilesField;

declare_rule! {
    /// A `Build-Profiles` restriction formula names an unknown build profile.
    ///
    /// Checks the `Build-Profiles` field of every installable of
    /// `debian/control`. A value that does not match the restriction-list
    /// grammar (`<a b> <!c>`) is skipped. A profile is known when it is in
    /// `data/fields/build-profiles`, or matches `pkg.<source>.<anything>`; a
    /// leading `!` is ignored. Extra is the profile, then `(in section for
    /// <installable>)`. The Location is `debian/control` at the line the field
    /// starts on, found by scanning the text (no line when it cannot be read).
    InvalidProfileNameInBuildProfilesField,
    code = "LN1017",
    tag = "invalid-profile-name-in-build-profiles-field",
    default_selected = true,
    experimental = false,
}

static KNOWN_PROFILES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    crate::lint::data::lines(include_str!("../../../data/fields/build-profiles")).collect()
});
static PACKAGE_PROFILE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^pkg\.[a-z0-9][a-z0-9+.-]+\..").expect("valid regex"));

impl SourceRule for InvalidProfileNameInBuildProfilesField {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        for restriction in restrictions(ctx) {
            for profile in &restriction.profiles {
                if KNOWN_PROFILES.contains(profile.as_str()) || PACKAGE_PROFILE.is_match(profile) {
                    continue;
                }
                ctx.diagnostic(
                    Diagnostic::error(format!(
                        "{profile} (in section for {})",
                        restriction.installable
                    ))
                    .with_location(build_profiles_location(&restriction)),
                );
            }
        }
    }
}

/// The restriction formula of one installable's `Build-Profiles` field.
pub(super) struct Restriction {
    /// The installable's `Package` name.
    pub(super) installable: String,
    /// Line the field starts on in `debian/control`, when it can be found.
    pub(super) line: Option<u32>,
    /// Every profile of the formula in file order, a leading `!` removed.
    pub(super) profiles: Vec<String>,
}

/// The `Build-Profiles` formulas of the installables that match lintian's
/// restriction-list grammar, in file order.
pub(super) fn restrictions(ctx: &mut SourceContext<'_>) -> Vec<Restriction> {
    let Some(control) = ctx.debian_control() else {
        return Vec::new();
    };
    let raw: Vec<(String, String)> = control
        .installables()
        .iter()
        .filter_map(|installable| {
            let value = installable
                .fields()
                .untrimmed_value("Build-Profiles")?
                .trim();
            (!value.is_empty()).then(|| (installable.name().to_string(), value.to_string()))
        })
        .collect();
    let text = control_text(ctx);
    raw.into_iter()
        .filter_map(|(installable, value)| {
            let profiles = profiles(&value)?;
            let line = text
                .as_deref()
                .and_then(|text| field_line(text, &installable, "Build-Profiles"));
            Some(Restriction {
                installable,
                line,
                profiles,
            })
        })
        .collect()
}

/// Location of a `Build-Profiles` finding: `debian/control` at the field.
pub(super) fn build_profiles_location(restriction: &Restriction) -> Location {
    Location {
        path: PathBuf::from("debian/control"),
        line: restriction.line,
        column: None,
    }
}

static FORMULA: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*<!?[^\s<>]+(?:\s+!?[^\s<>]+)*>(?:\s+<!?[^\s<>]+(?:\s+!?[^\s<>]+)*>)*\s*$")
        .expect("valid regex")
});
static OUTER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*<(.*)>\s*$").expect("valid regex"));
static LIST_SEPARATOR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r">\s+<").expect("valid regex"));

/// The profiles of a formula, as Check::Debian::Control::Field::BuildProfiles
/// splits it. `None` when the value is not a restriction formula.
fn profiles(value: &str) -> Option<Vec<String>> {
    if !FORMULA.is_match(value) {
        return None;
    }
    // Perl's `.` stops at a line break, so a multiline value keeps its `<` and `>`.
    let inner = OUTER.captures(value).map_or(value, |captures| {
        captures.get(1).map_or(value, |m| m.as_str())
    });
    Some(
        LIST_SEPARATOR
            .split(inner)
            .flat_map(str::split_whitespace)
            .map(|profile| profile.strip_prefix('!').unwrap_or(profile).to_string())
            .collect(),
    )
}

fn control_text(ctx: &mut SourceContext<'_>) -> Option<String> {
    let files = ctx.files()?;
    let entry = files.get("debian/control")?;
    String::from_utf8(files.read(entry).ok()?).ok()
}

/// 1-based line where `field` starts in the paragraph whose `Package` is `package`.
pub(super) fn field_line(text: &str, package: &str, field: &str) -> Option<u32> {
    let mut paragraphs: Vec<Vec<(u32, &str)>> = vec![Vec::new()];
    for (index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            paragraphs.push(Vec::new());
        } else if let Some(paragraph) = paragraphs.last_mut() {
            paragraph.push((u32::try_from(index).unwrap_or(u32::MAX) + 1, line));
        }
    }
    let paragraph = paragraphs.iter().find(|paragraph| {
        paragraph
            .iter()
            .any(|(_, line)| field_value(line, "Package") == Some(package))
    })?;
    paragraph
        .iter()
        .find(|(_, line)| field_value(line, field).is_some())
        .map(|(number, _)| *number)
}

/// The value of a `name: value` line; `None` for continuation and comment lines.
fn field_value<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    if line.starts_with([' ', '\t', '#']) {
        return None;
    }
    let (key, value) = line.split_once(':')?;
    key.eq_ignore_ascii_case(name).then(|| value.trim())
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const SOURCE: &str = "Source: example\nMaintainer: Example <ex@example.com>\n\n";

    fn control(build_profiles: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/control",
            TestFile::new(format!(
                "{SOURCE}Package: example\nArchitecture: all\n\
                 Build-Profiles: {build_profiles}\nDescription: example\n extra\n"
            )),
        )]
    }

    #[test]
    fn source() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            control("<cross> <!nocheck>"),
            control("<stage1 !nodoc> <pkg.example.foo>"),
            control("<!stage1> <nocheck> <pkg.example-1.x>"),
            control("not-a-formula"),
            control("<cross"),
            vec![(
                "debian/control",
                TestFile::new(format!("{SOURCE}Package: example\nArchitecture: all\n")),
            )],
        ];
        let fail = vec![
            control("<unknown>"),
            control("<!stage1 !unknown> <cross>"),
            control("<pkg.example>"),
            control("<cross> <bogus nocheck>"),
        ];
        SourceTreeTester::new(InvalidProfileNameInBuildProfilesField, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let dsc = || crate::lint::dsc::Dsc::parse("Source: example\n").expect("Dsc");
        let pass = vec![SourcePackageCase::from(control("<cross>")).dsc(dsc())];
        let fail = vec![SourcePackageCase::from(control("<unknown>")).dsc(dsc())];
        SourcePackageTester::new(InvalidProfileNameInBuildProfilesField, pass, fail)
            .test_and_snapshot();
    }
}
