use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::external_tool::{self, Sandbox};
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::rule::BinaryPackageRule;

pub struct AppstreamMetadataValidationFailed;

declare_rule! {
    /// `appstreamcli validate-tree` reports problems with the AppStream
    /// metadata of the Binary package.
    ///
    /// Only when `usr/share/metainfo/` exists. The installed tree of the
    /// package is written to a scratch directory and `appstreamcli validate-tree
    /// --format=yaml --no-net .` runs there. Reported when its output (stdout
    /// and stderr together) says `Passed: no`. Extra is `Problems reported by
    /// "appstreamcli validate-tree".`; there is no Location. The Rule reports
    /// nothing when `appstreamcli` is not installed. Applies to udebs too.
    AppstreamMetadataValidationFailed,
    code = "LN1467",
    tag = "appstream-metadata-validation-failed",
    default_selected = true,
    experimental = false,
}

/// lintian's `m/['"]?Passed['"]?: ['"]?no['"]?/` on the merged output.
static PASSED_NO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"['"]?Passed['"]?: ['"]?no['"]?"#).expect("valid regex"));

impl BinaryPackageRule for AppstreamMetadataValidationFailed {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let has_metainfo = files
            .get("usr/share/metainfo")
            .is_some_and(|entry| entry.kind() == EntryKind::Directory);
        if has_metainfo && validation_reports_failure(files) {
            ctx.diagnostic(Diagnostic::warn(
                r#"Problems reported by "appstreamcli validate-tree"."#,
            ));
        }
    }
}

/// Runs `appstreamcli validate-tree` on the whole installed tree and reports
/// whether it says `Passed: no`. `false` when the tool is missing or cannot
/// run.
fn validation_reports_failure(files: &FileTree) -> bool {
    let Some(tool) = external_tool::find("appstreamcli") else {
        return false;
    };
    let entries: Vec<&Entry> = files.entries().iter().collect();
    let Ok(sandbox) = Sandbox::new(files, &entries) else {
        return false;
    };
    let args = [
        std::ffi::OsStr::new("validate-tree"),
        std::ffi::OsStr::new("--format=yaml"),
        std::ffi::OsStr::new("--no-net"),
        std::ffi::OsStr::new("."),
    ];
    let Ok(captured) = sandbox.run(
        &tool,
        &args,
        Path::new(""),
        &[],
        external_tool::DEFAULT_TIMEOUT,
    ) else {
        return false;
    };
    let mut merged = captured.stdout;
    merged.extend_from_slice(&captured.stderr);
    PASSED_NO.is_match(&String::from_utf8_lossy(&merged))
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn metainfo(content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![
            ("usr/share/metainfo", TestFile::directory()),
            (
                "usr/share/metainfo/org.example.app.metainfo.xml",
                TestFile::new(content),
            ),
        ])
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            BinaryPackageCase::from(vec![("usr/share/doc/pkg/README", TestFile::new("x\n"))]),
        ];
        let fail = vec![metainfo("<component>\n")];
        BinaryPackageTester::new(AppstreamMetadataValidationFailed, pass, fail).test();
    }
}
