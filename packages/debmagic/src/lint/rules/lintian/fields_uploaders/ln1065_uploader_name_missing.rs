use std::path::PathBuf;

use regex::Regex;
use std::sync::LazyLock;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct UploaderNameMissing;

declare_rule! {
    /// The `Uploaders` field of the source paragraph has an empty entry (`,\s*,`).
    ///
    /// The double comma is what lintian's `fields/uploaders` Check calls a
    /// missing name: `Uploaders: A <a@example.org>, , B <b@example.org>`.
    /// Read from `debian/control` (the source paragraph), and only when that
    /// field is declared; a Source package also needs its `.dsc` to declare
    /// `Uploaders`. Extra is `you have used a double comma`. Location is
    /// `debian/control` at the line where the `Uploaders` field starts. The
    /// line is omitted when `debian/control` is not in the File tree. Applies
    /// to the Source tree and Source package.
    UploaderNameMissing,
    code = "LN1065",
    tag = "uploader-name-missing",
    default_selected = true,
    experimental = false,
}

static DOUBLE_COMMA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r",\s*,").expect("valid regex"));

const MESSAGE: &str = "you have used a double comma";

impl SourceRule for UploaderNameMissing {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        if ctx
            .dsc()
            .is_some_and(|dsc| !dsc.fields().declares("Uploaders"))
        {
            return;
        }
        let Some(uploaders) = ctx
            .debian_control()
            .and_then(|control| control.source().unfolded_value("Uploaders"))
        else {
            return;
        };
        if !DOUBLE_COMMA.is_match(&uploaders) {
            return;
        }
        let line = uploaders_line(ctx);
        ctx.diagnostic(Diagnostic::error(MESSAGE).with_location(Location {
            path: PathBuf::from("debian/control"),
            line,
            column: None,
        }));
    }
}

/// 1-based line of the `Uploaders:` field in the first (source) paragraph.
fn uploaders_line(ctx: &mut SourceContext<'_>) -> Option<u32> {
    let files = ctx.files()?;
    let entry = files.get("debian/control")?;
    let text = String::from_utf8_lossy(&files.read(entry).ok()?).into_owned();
    text.lines()
        .take_while(|line| !line.trim().is_empty())
        .position(|line| {
            line.get(.."Uploaders:".len())
                .is_some_and(|key| key.eq_ignore_ascii_case("Uploaders:"))
        })
        .and_then(|index| u32::try_from(index + 1).ok())
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn control(uploaders: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: example\nPriority: optional\nSection: misc\n\
                 Maintainer: A <a@example.com>\nUploaders: {uploaders}\n\n\
                 Package: example\nArchitecture: all\nDescription: example\n extra\n"
            )),
        )]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            control("B <b@example.com>"),
            control("B <b@example.com>, C <c@example.com>"),
            // A comma with no empty entry before the next.
            control("B <b@example.com>,\n C <c@example.com>"),
        ];
        let fail = vec![
            control("B <b@example.com>, , C <c@example.com>"),
            control("B <b@example.com>,, C <c@example.com>"),
            control("B <b@example.com>,\n , C <c@example.com>"),
        ];
        SourceTreeTester::new(UploaderNameMissing, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let dsc = |uploaders: &str| {
            Dsc::parse(&format!(
                "Source: example\nMaintainer: A <a@example.com>\nUploaders: {uploaders}\n"
            ))
            .expect("Dsc")
        };
        let no_uploaders =
            Dsc::parse("Source: example\nMaintainer: A <a@example.com>\n").expect("Dsc");
        let pass = vec![
            SourcePackageCase::from(control("B <b@example.com>")).dsc(dsc("B <b@example.com>")),
            // The Dsc gates: without Uploaders in the .dsc, nothing is read.
            SourcePackageCase::from(control("B <b@example.com>,, C <c@example.com>"))
                .dsc(no_uploaders),
        ];
        // The value comes from debian/control; the .dsc only gates.
        let fail = vec![
            SourcePackageCase::from(control("B <b@example.com>, , C <c@example.com>"))
                .dsc(dsc("B <b@example.com>")),
        ];
        SourcePackageTester::new(UploaderNameMissing, pass, fail).test_and_snapshot();
    }
}
