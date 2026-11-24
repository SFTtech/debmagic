use std::path::Path;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Severity, SourceRule};

use super::ln0396_format_3_0_but_debian_changes_patch::{is_quilt_format, source_format};
use super::ln0706_patch_file_present_but_not_mentioned_in_series::BUILD_DEPENDS_ALL;

pub struct QuiltPatchUsesDpatchExtension;

declare_rule! {
    /// A patch named in `debian/patches/series` has the `.dpatch` extension,
    /// which refers to the deprecated dpatch patching system.
    ///
    /// Runs under the gate of `quilt-patch-with-non-standard-options` (LN1207):
    /// Format is `3.x (quilt)` or `Build-Depends-All` satisfies `quilt`, and
    /// `debian/patches/series` must be an open file. Every patch line counts,
    /// whether or not the named file exists, and a commented-out line does not.
    /// Extra is the patch name. No Location, as lintian gives none. Reported at
    /// info severity. Applies to the Source tree and Source package.
    QuiltPatchUsesDpatchExtension,
    code = "LN1704",
    tag = "quilt-patch-uses-dpatch-extension",
    default_selected = false,
    experimental = false,
}

impl SourceRule for QuiltPatchUsesDpatchExtension {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let quilt_format = is_quilt_format(&source_format(ctx));
        let builds_with_quilt = ctx.debian_control().is_some_and(|control| {
            control
                .source()
                .relations_and(BUILD_DEPENDS_ALL)
                .satisfies("quilt")
        });
        if !quilt_format && !builds_with_quilt {
            return;
        }
        let series_path = Path::new("debian/patches/series");
        let Some((_, series)) = ctx
            .quilt_series()
            .into_iter()
            .find(|(path, _)| path == series_path)
        else {
            return;
        };
        for patch in series.patches() {
            if patch.name().ends_with(".dpatch") {
                ctx.diagnostic(Diagnostic::warn(patch.name()).with_severity(Severity::Info));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    const CONTROL_QUILT: &str = "\
Source: example
Maintainer: Maintainer <m@example.com>
Build-Depends: debhelper-compat (= 13), quilt

Package: example
Architecture: all
Description: example
 extra
";

    const CONTROL_PLAIN: &str = "\
Source: example
Maintainer: Maintainer <m@example.com>
Build-Depends: debhelper-compat (= 13)

Package: example
Architecture: all
Description: example
 extra
";

    fn tree(format: &str, control: &str, series: &str) -> Vec<(String, TestFile)> {
        vec![
            (
                "debian/source/format".to_string(),
                TestFile::new(format!("{format}\n")),
            ),
            ("debian/control".to_string(), TestFile::new(control)),
            ("debian/patches/series".to_string(), TestFile::new(series)),
        ]
    }

    #[test]
    fn source() {
        let pass = vec![
            tree("3.0 (quilt)", CONTROL_QUILT, "a.diff\nb.patch\n"),
            tree("3.0 (quilt)", CONTROL_QUILT, "# old.dpatch\n\na.patch\n"),
            // Neither quilt format nor quilt build dependency: not checked.
            tree("1.0", CONTROL_PLAIN, "a.dpatch\n"),
        ];
        let fail = vec![
            tree("3.0 (quilt)", CONTROL_QUILT, "a.dpatch\nb.patch\n"),
            tree("1.0", CONTROL_QUILT, "a.dpatch -p1\nc.dpatch\n"),
        ];
        SourceTreeTester::new(QuiltPatchUsesDpatchExtension, pass, fail).test_and_snapshot();
    }
}
