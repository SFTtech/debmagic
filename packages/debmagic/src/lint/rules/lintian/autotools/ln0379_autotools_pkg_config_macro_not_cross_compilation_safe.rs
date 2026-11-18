use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::SourceRule;

pub struct AutotoolsPkgConfigMacroNotCrossCompilationSafe;

declare_rule! {
    /// `configure.ac` or `configure.in` looks up `pkg-config` with `AC_PATH_PROG`.
    ///
    /// A `dnl` comment line is skipped. Extra is the macro name. The Location
    /// is that line of the patched file.
    AutotoolsPkgConfigMacroNotCrossCompilationSafe,
    code = "LN0379",
    tag = "autotools-pkg-config-macro-not-cross-compilation-safe",
    default_selected = true,
    experimental = false,
}

static CONFIGURE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"configure\.(?:in|ac)$").expect("valid regex"));
static DNL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*dnl").expect("valid regex"));
static MACRO: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(AC_PATH_PROG)\s*\([^,]+,\s*\[?pkg-config\]?\s*,").expect("valid regex")
});

impl SourceRule for AutotoolsPkgConfigMacroNotCrossCompilationSafe {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .query()
            .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
            .into_iter()
            .filter(|entry| {
                entry
                    .path()
                    .to_str()
                    .is_some_and(|path| CONFIGURE.is_match(path))
            })
            .map(|entry| entry.path().to_path_buf())
            .collect();
        let mut hits = Vec::new();
        for path in paths {
            let Some(entry) = files.get(&path) else {
                continue;
            };
            let Ok(bytes) = files.read(entry) else {
                continue;
            };
            let text = String::from_utf8_lossy(&bytes);
            for (index, line) in text.lines().enumerate() {
                if DNL.is_match(line) {
                    continue;
                }
                let Some(captures) = MACRO.captures(line) else {
                    continue;
                };
                let line_number = u32::try_from(index + 1).unwrap_or(u32::MAX);
                hits.push((
                    path.clone(),
                    line_number,
                    captures
                        .get(1)
                        .map(|m| m.as_str())
                        .unwrap_or("")
                        .to_string(),
                ));
            }
        }
        for (path, line, macro_name) in hits {
            ctx.diagnostic(Diagnostic::warn(macro_name).with_location(Location {
                path,
                line: Some(line),
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "Source: example\nMaintainer: Example <ex@example.com>\nStandards-Version: 4.7.2\n\nPackage: example\nArchitecture: all\nDescription: example\n extra\n";

    fn tree(extra: &[(&str, &str)]) -> Vec<(String, TestFile)> {
        let mut files = vec![("debian/control".to_string(), TestFile::from(CONTROL))];
        for (path, content) in extra {
            files.push(((*path).to_string(), TestFile::new(*content)));
        }
        files
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(&[]),
            tree(&[(
                "configure.ac",
                "AC_PATH_TOOL([PKG_CONFIG], pkg-config, no)\n",
            )]),
            tree(&[(
                "configure.ac",
                "dnl AC_PATH_PROG([PKG_CONFIG], pkg-config, no)\n",
            )]),
            tree(&[("configure", "AC_PATH_PROG([PKG_CONFIG], pkg-config, no)\n")]),
        ];
        let fail = vec![
            tree(&[(
                "configure.ac",
                "AC_PATH_PROG([PKG_CONFIG], [pkg-config], no)\n",
            )]),
            tree(&[(
                "src/configure.in",
                "dnl skip\nAC_PATH_PROG([PKG_CONFIG], pkg-config, no)\n",
            )]),
        ];
        SourceTreeTester::new(AutotoolsPkgConfigMacroNotCrossCompilationSafe, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree(&[]))];
        let fail = vec![SourcePackageCase::from(tree(&[(
            "configure.ac",
            "AC_PATH_PROG([PKG_CONFIG], pkg-config, no)\n",
        )]))];
        SourcePackageTester::new(AutotoolsPkgConfigMacroNotCrossCompilationSafe, pass, fail)
            .test_and_snapshot();
    }
}
