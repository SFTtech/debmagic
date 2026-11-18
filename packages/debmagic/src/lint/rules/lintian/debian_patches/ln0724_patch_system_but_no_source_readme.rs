use std::path::{Component, Path};

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::rule::SourceRule;

use super::ln0396_format_3_0_but_debian_changes_patch::{is_quilt_format, source_format};
use super::ln0706_patch_file_present_but_not_mentioned_in_series::BUILD_DEPENDS_ALL;

pub struct PatchSystemButNoSourceReadme;

declare_rule! {
    /// A Format that is not `3.x (quilt)` whose Build-Depends-All satisfies
    /// `quilt`, but without `debian/README.source`.
    ///
    /// Format comes from the Dsc when there is one, otherwise the first line
    /// of `debian/source/format`. `debian/README.source` counts when it is
    /// any entry, or a symlink that resolves inside the tree; a dangling
    /// symlink does not. Extra is empty. No Location. Applies to the Source
    /// tree and Source package.
    PatchSystemButNoSourceReadme,
    code = "LN0724",
    tag = "patch-system-but-no-source-readme",
    default_selected = true,
    experimental = false,
}

impl SourceRule for PatchSystemButNoSourceReadme {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        if is_quilt_format(&source_format(ctx)) {
            return;
        }
        let uses_quilt = ctx.debian_control().is_some_and(|control| {
            control
                .source()
                .relations_and(BUILD_DEPENDS_ALL)
                .satisfies("quilt")
        });
        if !uses_quilt {
            return;
        }
        let Some(files) = ctx.files() else {
            return;
        };
        if has_readme_source(files) {
            return;
        }
        ctx.diagnostic(Diagnostic::warn(""));
    }
}

/// Whether `debian/README.source` resolves to an entry in the tree, following
/// relative symlinks.
pub(crate) fn has_readme_source(files: &FileTree) -> bool {
    let Some(mut entry) = files.get("debian/README.source") else {
        return false;
    };
    for _ in 0..8 {
        if entry.kind() != EntryKind::Symlink {
            return true;
        }
        let Some(target) = entry.link_target() else {
            return false;
        };
        let mut resolved = entry.path().parent().unwrap_or(Path::new("")).to_path_buf();
        for component in target.components() {
            match component {
                Component::Normal(part) => resolved.push(part),
                Component::CurDir => {}
                Component::ParentDir => {
                    if !resolved.pop() {
                        return false;
                    }
                }
                Component::RootDir | Component::Prefix(_) => return false,
            }
        }
        let Some(next) = files.get(&resolved) else {
            return false;
        };
        entry = next;
    }
    false
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "\
Source: example
Maintainer: Maintainer <m@example.com>

Package: example
Architecture: all
Description: example
 extra
";

    const CONTROL_QUILT: &str = "\
Source: example
Maintainer: Maintainer <m@example.com>
Build-Depends: debhelper-compat (= 13), quilt

Package: example
Architecture: all
Description: example
 extra
";

    const CONTROL_INDEP_QUILT: &str = "\
Source: example
Maintainer: Maintainer <m@example.com>
Build-Depends-Indep: quilt

Package: example
Architecture: all
Description: example
 extra
";

    fn tree(
        format: Option<&str>,
        control: &str,
        extra: &[(&str, TestFile)],
    ) -> Vec<(String, TestFile)> {
        let mut files = vec![("debian/control".to_string(), TestFile::new(control))];
        if let Some(format) = format {
            files.push((
                "debian/source/format".to_string(),
                TestFile::new(format!("{format}\n")),
            ));
        }
        for (path, file) in extra {
            files.push(((*path).to_string(), file.clone()));
        }
        files
    }

    fn file(content: &str) -> TestFile {
        TestFile::new(content)
    }

    #[test]
    fn test() {
        let pass = vec![
            // No patch system.
            tree(Some("3.0 (native)"), CONTROL, &[]),
            tree(Some("1.0"), CONTROL, &[]),
            // Quilt format needs no README.source.
            tree(Some("3.0 (quilt)"), CONTROL_QUILT, &[]),
            tree(Some("3.0 (quilt)"), CONTROL, &[]),
            // README.source as a file, a directory, or a symlink into the tree.
            tree(
                Some("3.0 (native)"),
                CONTROL_QUILT,
                &[("debian/README.source", file("readme"))],
            ),
            tree(
                Some("3.0 (native)"),
                CONTROL_QUILT,
                &[("debian/README.source", TestFile::directory())],
            ),
            tree(
                Some("3.0 (native)"),
                CONTROL_QUILT,
                &[
                    ("debian/README.source", TestFile::symlink("source-readme")),
                    ("debian/source-readme", file("readme")),
                ],
            ),
            // Format 1.0 with the quilt build dependency in Build-Depends-Indep.
            tree(
                Some("1.0"),
                CONTROL_INDEP_QUILT,
                &[("debian/README.source", file("readme"))],
            ),
        ];
        let fail = vec![
            tree(Some("3.0 (native)"), CONTROL_QUILT, &[]),
            // Format 2.0 is not quilt format, so the README is still required.
            tree(Some("2.0"), CONTROL_QUILT, &[]),
            // No format file.
            tree(None, CONTROL_QUILT, &[]),
            // Build-Depends-Indep counts.
            tree(Some("3.0 (native)"), CONTROL_INDEP_QUILT, &[]),
            // A dangling README.source symlink does not count.
            tree(
                Some("3.0 (native)"),
                CONTROL_QUILT,
                &[("debian/README.source", TestFile::symlink("missing"))],
            ),
        ];
        SourceTreeTester::new(PatchSystemButNoSourceReadme, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let files = tree(Some("1.0"), CONTROL_QUILT, &[]);
        let pass = vec![SourcePackageCase::from(files.clone()).dsc(
            Dsc::parse("Format: 3.0 (quilt)\nVersion: 1.0-1\nSource: example\n").expect("dsc"),
        )];
        let fail = vec![SourcePackageCase::from(files).dsc(
            Dsc::parse("Format: 3.0 (native)\nVersion: 1.0-1\nSource: example\n").expect("dsc"),
        )];
        SourcePackageTester::new(PatchSystemButNoSourceReadme, pass, fail).test_and_snapshot();
    }
}
