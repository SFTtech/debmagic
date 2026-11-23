use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::diffstat::Diffstat;
use crate::lint::rule::{Severity, SourceRule};

use super::ln0396_format_3_0_but_debian_changes_patch::{is_quilt_format, source_format};
use super::ln0706_patch_file_present_but_not_mentioned_in_series::BUILD_DEPENDS_ALL;

pub struct DirectChangesInDiffButNoPatchSystem;

declare_rule! {
    /// The Debian diff (`<name>_<version>.diff.gz`) changes a file outside
    /// `debian/`, and no patch system is in use. Extra is the first such file
    /// in sorted order, with ` and N more` when there are more; lintian's
    /// hash order is not reproduced, so with several files the first one
    /// named may differ from lintian's. The Location is empty.
    ///
    /// A patch system is in use when the Format is `3.x (quilt)` or the
    /// Build-Depends-All satisfy `quilt`. The file list is
    /// [`SourceContext::diffstat`]: the diff listed beside the `.dsc`, and
    /// empty when it is absent or unreadable. A Source tree has no diff, so
    /// the Rule never reports there. Applies to the Source package only in
    /// practice. Severity is pedantic and the Tag is not selected by default.
    DirectChangesInDiffButNoPatchSystem,
    code = "LN1706",
    tag = "direct-changes-in-diff-but-no-patch-system",
    default_selected = false,
    experimental = false,
}

impl SourceRule for DirectChangesInDiffButNoPatchSystem {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let diffstat = ctx.diffstat();
        let Some(extra) = direct_changes_extra(&diffstat) else {
            return;
        };
        if uses_patch_system(ctx) {
            return;
        }
        ctx.diagnostic(Diagnostic::warn(extra).with_severity(Severity::Pedantic));
    }
}

/// The Extra of the Debian diff's direct changes, shared with
/// `patch-system-but-direct-changes-in-diff`: the first file outside `debian/`
/// in sorted order, with ` and N more` when there are more. `None` when the
/// diff changes no file outside `debian/`.
pub(crate) fn direct_changes_extra(diffstat: &Diffstat) -> Option<String> {
    let direct: Vec<&String> = diffstat
        .files()
        .iter()
        .filter(|path| !path.starts_with("debian/"))
        .collect();
    let first = direct.first()?;
    Some(match direct.len() {
        1 => first.to_string(),
        count => format!("{first} and {} more", count - 1),
    })
}

/// Whether lintian's `patch-system` Tag fires: the Format matches
/// `3.x (quilt)`, or the Build-Depends-All satisfy `quilt`. Shared by the
/// `debian/patches` Tags that depend on a patch system being in use.
pub(crate) fn uses_patch_system(ctx: &mut SourceContext<'_>) -> bool {
    if is_quilt_format(&source_format(ctx)) {
        return true;
    }
    ctx.debian_control().is_some_and(|control| {
        control
            .source()
            .relations_and(BUILD_DEPENDS_ALL)
            .satisfies("quilt")
    })
}

#[cfg(test)]
pub(super) mod tests {
    use std::io::Write;

    use flate2::Compression;
    use flate2::write::GzEncoder;

    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{
        SourcePackageCase, SourcePackageTester, SourceTreeTester, TestCase, TestFile,
    };

    use super::*;

    pub(in super::super) const CONTROL: &str = "\
Source: example
Maintainer: Maintainer <m@example.com>

Package: example
Architecture: all
Description: example
 extra
";

    pub(in super::super) const CONTROL_QUILT: &str = "\
Source: example
Maintainer: Maintainer <m@example.com>
Build-Depends: debhelper-compat (= 13), quilt

Package: example
Architecture: all
Description: example
 extra
";

    /// The `.dsc` of version 1.0-1 in `format`. Its Files list the diff when
    /// `listed`, and never an orig tarball (the Rule does not read it).
    pub(in super::super) fn dsc(format: &str, listed: bool) -> Dsc {
        let mut text = format!(
            "Source: example\nVersion: 1.0-1\nFormat: {format}\nFiles:\n 00000000000000000000000000000000 0 example_1.0.orig.tar.gz\n"
        );
        if listed {
            text.push_str(" 00000000000000000000000000000000 0 example_1.0-1.diff.gz\n");
        }
        Dsc::parse(&text).expect("dsc paragraph")
    }

    /// A gzipped unified diff that changes each path in `paths`.
    pub(in super::super) fn diff_gz(paths: &[&str]) -> TestFile {
        let mut text = String::new();
        for path in paths {
            text.push_str(&format!(
                "--- example-1.0.orig/{path}\n+++ example-1.0/{path}\n@@ -1 +1 @@\n-a\n+b\n"
            ));
        }
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(text.as_bytes()).expect("gzip write");
        TestFile::from_bytes(encoder.finish().expect("gzip finish"))
    }

    /// A Source package case: patched `debian/control` (`control`), the `.dsc`
    /// in `format`, and the diff beside it under `diff_name`.
    pub(in super::super) fn case(
        control: &str,
        format: &str,
        diff_name: &str,
        diff: TestFile,
        listed: bool,
    ) -> SourcePackageCase {
        SourcePackageCase::from(vec![("debian/control", TestFile::new(control))])
            .siblings(vec![(diff_name, diff)])
            .dsc(dsc(format, listed))
    }

    #[test]
    fn source_tree_has_no_diff() {
        // A Source tree has no `.diff.gz`, so it never reports.
        let pass = vec![TestCase::from(vec![
            ("debian/control", TestFile::new(CONTROL)),
            ("README", TestFile::new("readme")),
        ])];
        SourceTreeTester::new(
            DirectChangesInDiffButNoPatchSystem,
            pass,
            Vec::<TestCase>::new(),
        )
        .test();
    }

    #[test]
    fn source_package() {
        let diff = "example_1.0-1.diff.gz";
        let pass = vec![
            // Only debian/ changed.
            case(CONTROL, "1.0", diff, diff_gz(&["debian/changelog"]), true),
            // A patch system: Format 3.0 (quilt) ...
            case(CONTROL, "3.0 (quilt)", diff, diff_gz(&["README"]), true),
            // ... or quilt in Build-Depends-All.
            case(CONTROL_QUILT, "1.0", diff, diff_gz(&["README"]), true),
            // No diff listed in the .dsc, so no sibling is read.
            case(CONTROL, "1.0", diff, diff_gz(&["README"]), false),
            // A diff that is not gzip, or no diff at all, changes nothing.
            case(CONTROL, "1.0", diff, TestFile::new("not gzip"), true),
            SourcePackageCase::from(vec![("debian/control", TestFile::new(CONTROL))])
                .dsc(dsc("3.0 (native)", false)),
        ];
        let fail = vec![
            case(CONTROL, "1.0", diff, diff_gz(&["README"]), true),
            // Format 2.0 is no patch system; Build-Depends-All only with quilt.
            case(CONTROL, "2.0", diff, diff_gz(&["src/main.c"]), true),
            // Several files: the first in sorted order, and the count.
            case(
                CONTROL,
                "1.0",
                diff,
                diff_gz(&["src/main.c", "README", "debian/rules"]),
                true,
            ),
        ];
        SourcePackageTester::new(DirectChangesInDiffButNoPatchSystem, pass, fail)
            .test_and_snapshot();
    }
}
