use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::data;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

pub struct DebianControlHasObsoleteDbgPackage;

declare_rule! {
    /// An installable in `debian/control` is named `*-dbg`.
    ///
    /// Debug packages are generated automatically, so a `-dbg` package is
    /// obsolete. Installables whose name matches a pattern in lintian's
    /// `common/dbg-pkg` table are exempt: the `libc<N>-dbg` and
    /// `python<N>-*-dbg` packages that build against a debug runtime. Checks
    /// every installable paragraph of `debian/control`; the source paragraph is
    /// not checked. Extra is `(in section for <installable>) Package`. The
    /// Location is `debian/control` at the line the `Package` field starts on
    /// (no line when the file cannot be read). Reported at info severity.
    DebianControlHasObsoleteDbgPackage,
    code = "LN1346",
    tag = "debian-control-has-obsolete-dbg-package",
    default_selected = false,
    experimental = false,
}

/// Patterns of `common/dbg-pkg`, compiled once.
static LEGACY_DBG_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    data::lines(include_str!("../../../data/common/dbg-pkg"))
        .map(|pattern| Regex::new(pattern).expect("valid regex in common/dbg-pkg"))
        .collect()
});

impl SourceRule for DebianControlHasObsoleteDbgPackage {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let obsolete: Vec<String> = control
            .installables()
            .iter()
            .map(|installable| installable.name())
            .filter(|name| name.ends_with("-dbg"))
            .filter(|name| !LEGACY_DBG_PATTERNS.iter().any(|re| re.is_match(name)))
            .map(str::to_string)
            .collect();
        if obsolete.is_empty() {
            return;
        }
        let text = control_text(ctx);
        for name in obsolete {
            let line = text.as_deref().and_then(|text| package_line(text, &name));
            ctx.diagnostic(
                Diagnostic::warn(format!("(in section for {name}) Package"))
                    .with_severity(Severity::Info)
                    .with_location(Location {
                        path: PathBuf::from("debian/control"),
                        line,
                        column: None,
                    }),
            );
        }
    }
}

fn control_text(ctx: &mut SourceContext<'_>) -> Option<String> {
    let files = ctx.files()?;
    let entry = files.get("debian/control")?;
    String::from_utf8(files.read(entry).ok()?).ok()
}

/// 1-based line of the `Package` field in the paragraph whose `Package` is `package`.
fn package_line(text: &str, package: &str) -> Option<u32> {
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
        .find(|(_, line)| field_value(line, "Package").is_some())
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

    fn control(installables: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/control",
            TestFile::new(format!("{SOURCE}{installables}")),
        )]
    }

    #[test]
    fn source() {
        let pass = vec![
            control("Package: example\nArchitecture: any\nDescription: example\n extra\n"),
            control("Package: libc6-dbg\nArchitecture: any\nDescription: debug\n extra\n"),
            control("Package: python3-foo-dbg\nArchitecture: any\nDescription: debug\n extra\n"),
        ];
        let fail = vec![control(
            "Package: example-dbg\nArchitecture: any\nDescription: debug\n extra\n",
        )];
        SourceTreeTester::new(DebianControlHasObsoleteDbgPackage, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![SourcePackageCase::from(control(
            "Package: example\nArchitecture: any\nDescription: example\n extra\n",
        ))];
        let fail = vec![SourcePackageCase::from(control(
            "Package: example\nArchitecture: any\nDescription: example\n extra\n\nPackage: example-dbg\nArchitecture: any\nDescription: debug\n extra\n",
        ))];
        SourcePackageTester::new(DebianControlHasObsoleteDbgPackage, pass, fail)
            .test_and_snapshot();
    }
}
