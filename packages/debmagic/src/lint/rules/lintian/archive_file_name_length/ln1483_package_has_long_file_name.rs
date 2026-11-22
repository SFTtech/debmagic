use crate::declare_rule;
use crate::lint::context::{BinaryPackageContext, SourceContext};
use crate::lint::diagnostic::Diagnostic;
use crate::lint::group_context::{BuildinfoContext, ChangesContext};
use crate::lint::rule::{BinaryPackageRule, BuildinfoRule, ChangesRule, SourceRule};

use super::file_name_too_long;

pub struct PackageHasLongFileName;

declare_rule! {
    /// The file name of the Subject is too long: the name, measured as
    /// lintian does (a `+salsaci+N+N` suffix removed, the architecture length
    /// swapped for that of `kfreebsd-amd64`), is over 80 characters. The extra
    /// is the file name. Applies to the `.dsc`, the `.deb` or `.udeb`, the
    /// `.changes` and the `.buildinfo` of a Group; a Source tree has no file
    /// name and is not checked. Packages that declare `Auto-Built-Package`
    /// (the `-dbgsym` packages) are skipped.
    ///
    /// The architecture is the `Architecture` of the Subject: `source` for a
    /// `.dsc`, the control `Architecture` of a binary, the `Architecture` field
    /// of a `.changes` or `.buildinfo` as written (a multi-word value counts
    /// its spaces, as lintian's does).
    PackageHasLongFileName,
    code = "LN1483",
    tag = "package-has-long-file-name",
    default_selected = true,
    experimental = false,
}

/// The file name of a path, or `None` when it has none.
fn file_name(path: &std::path::Path) -> Option<&str> {
    path.file_name()?.to_str()
}

impl SourceRule for PackageHasLongFileName {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        // A Source tree has no `.dsc`, so no file name to measure.
        let Some(dsc) = ctx.dsc() else {
            return;
        };
        if dsc.fields().declares("Auto-Built-Package") {
            return;
        }
        let Some(name) = file_name(ctx.path()) else {
            return;
        };
        if file_name_too_long(name, "source") {
            ctx.diagnostic(Diagnostic::warn(name));
        }
    }
}

impl BinaryPackageRule for PackageHasLongFileName {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let fields = ctx.installation_control().fields();
        if fields.declares("Auto-Built-Package") {
            return;
        }
        let Some(name) = file_name(ctx.path()) else {
            return;
        };
        let architecture = fields.get("Architecture").unwrap_or("").trim();
        if file_name_too_long(name, architecture) {
            ctx.diagnostic(Diagnostic::warn(name));
        }
    }
}

impl ChangesRule for PackageHasLongFileName {
    fn run(&self, ctx: &mut ChangesContext<'_>) {
        let changes = ctx.changes();
        if changes.fields().declares("Auto-Built-Package") {
            return;
        }
        let Some(name) = file_name(ctx.path()) else {
            return;
        };
        let architecture = changes.architecture().unwrap_or("");
        if file_name_too_long(name, architecture) {
            ctx.diagnostic(Diagnostic::warn(name));
        }
    }
}

impl BuildinfoRule for PackageHasLongFileName {
    fn run(&self, ctx: &mut BuildinfoContext<'_>) {
        let buildinfo = ctx.buildinfo();
        if buildinfo.fields().declares("Auto-Built-Package") {
            return;
        }
        let Some(name) = file_name(ctx.path()) else {
            return;
        };
        let architecture = buildinfo.architecture().unwrap_or("");
        if file_name_too_long(name, architecture) {
            ctx.diagnostic(Diagnostic::warn(name));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{ChangesCase, ChangesTester, SourcePackageCase, SourcePackageTester};

    use super::*;

    // The file names of the native fixtures are fixed by the testers (`example.dsc`,
    // `test.changes`), so the length cases are covered by the helper's unit tests;
    // these pass cases show the Rule stays quiet on short names.

    #[test]
    fn short_names_pass() {
        let changes = vec![
            ChangesCase::from("Source: example\nVersion: 1.0-1\nArchitecture: source all\n"),
            ChangesCase::from(
                "Source: example\nVersion: 1.0-1\nArchitecture: amd64\nAuto-Built-Package: debug-symbols\n",
            ),
        ];
        ChangesTester::new(PackageHasLongFileName, changes, Vec::new()).test();
        let dsc = vec![SourcePackageCase::from(vec![(
            "debian/control",
            "Source: example\n\nPackage: example\nArchitecture: all\nDescription: example\n extra\n",
        )])];
        SourcePackageTester::new(PackageHasLongFileName, dsc, Vec::new()).test();
    }

    #[test]
    fn length_limit_is_on_the_adjusted_length() {
        // `all` is 3 characters, so the adjusted length is the name's length
        // plus 11: a 69-character name is at the limit, 70 is over it.
        let deb = |length: usize| format!("{}_1.0_all.deb", "a".repeat(length - 12));
        assert!(!file_name_too_long(&deb(69), "all"));
        assert!(file_name_too_long(&deb(70), "all"));
        // `source` is 6 characters: the adjusted length is the name plus 8.
        let dsc = |length: usize| format!("{}.dsc", "a".repeat(length - 4));
        assert!(!file_name_too_long(&dsc(72), "source"));
        assert!(file_name_too_long(&dsc(73), "source"));
    }

    #[test]
    fn salsaci_suffix_is_removed_before_measuring() {
        let plain = format!("{}_1.0_amd64.deb", "a".repeat(40));
        let salsaci = format!("{}+salsaci+1+2_1.0_amd64.deb", "a".repeat(40));
        assert_eq!(
            file_name_too_long(&plain, "amd64"),
            file_name_too_long(&salsaci, "amd64")
        );
        let long = format!("{}+salsaci+1+2_1.0_amd64.deb", "a".repeat(70));
        assert!(file_name_too_long(&long, "amd64"));
    }

    #[test]
    fn length_counts_characters() {
        // 57 `ä` (2 bytes each) and a 12-character tail: 69 characters.
        let name = format!("{}_1.0_all.deb", "ä".repeat(57));
        assert!(!file_name_too_long(&name, "all"));
    }
}
