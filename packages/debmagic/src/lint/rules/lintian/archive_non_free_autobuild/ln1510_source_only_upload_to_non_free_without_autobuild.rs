use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::group_context::GroupContext;
use crate::lint::rule::GroupRule;

pub struct SourceOnlyUploadToNonFreeWithoutAutobuild;

declare_rule! {
    /// Reports a source-only upload (the `.changes` has `Architecture:
    /// source`) of a non-free package that does not opt in to the autobuilders:
    /// the source paragraph of `debian/control` has no `XS-Autobuild` field, or
    /// has `XS-Autobuild: no`. The package is non-free when the `Section` of
    /// the source paragraph starts with `non-free/`, `non-free-firmware/`,
    /// `restricted/` or `multiverse/` (no `Section` counts as `main`). The
    /// extra is `(in the source paragraph) XS-Autobuild`, at the field's line in
    /// `debian/control` when it is there.
    ///
    /// A Group Rule: it needs the Group's `.changes` and its `.dsc`, so it
    /// does not run on a Group without both. The Diagnostic attaches to the
    /// Source package. Differences from lintian: a `debian/control` with a
    /// Deb822 syntax error is not looked at.
    SourceOnlyUploadToNonFreeWithoutAutobuild,
    code = "LN1510",
    tag = "source-only-upload-to-non-free-without-autobuild",
    default_selected = true,
    experimental = false,
}

const NON_FREE_AREAS: [&str; 4] = [
    "non-free/",
    "non-free-firmware/",
    "restricted/",
    "multiverse/",
];

impl GroupRule for SourceOnlyUploadToNonFreeWithoutAutobuild {
    fn run(&self, ctx: &mut GroupContext<'_>) {
        let source_only = ctx
            .changes()
            .is_some_and(|changes| changes.fields().get("Architecture") == Some("source"));
        if !source_only {
            return;
        }
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let source = control.source();
        let section = source.get("Section").unwrap_or("main");
        if !NON_FREE_AREAS.iter().any(|area| section.starts_with(area)) {
            return;
        }
        let field = "XS-Autobuild";
        if source.declares(field) && source.get(field) != Some("no") {
            return;
        }
        ctx.diagnostic(
            Diagnostic::error(format!("(in the source paragraph) {field}")).with_location(
                Location {
                    path: PathBuf::from("debian/control"),
                    line: source.position(field),
                    column: None,
                },
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{GroupCase, GroupTester, TestCase};

    use super::*;

    const DSC: &str = "Source: example\nVersion: 1.0\n";
    const SOURCE_UPLOAD: &str = "Source: example\nVersion: 1.0\nArchitecture: source\n";
    const FULL_UPLOAD: &str = "Source: example\nVersion: 1.0\nArchitecture: source all\n";

    fn group(changes: &str, source_paragraph: &str) -> GroupCase {
        GroupCase::new(DSC).changes(changes).source_files(TestCase::from(vec![(
            "debian/control",
            format!(
                "Source: example\n{source_paragraph}\nPackage: example\nArchitecture: all\nDescription: x\n"
            )
            .as_str(),
        )]))
    }

    #[test]
    fn group_rule() {
        let pass = vec![
            // No `.changes`: not an upload.
            GroupCase::new(DSC),
            // Free packages need no autobuild.
            group(SOURCE_UPLOAD, "Section: devel\n"),
            group(SOURCE_UPLOAD, ""),
            // An upload with binaries is built already.
            group(FULL_UPLOAD, "Section: non-free/devel\n"),
            // Opted in.
            group(
                SOURCE_UPLOAD,
                "Section: non-free/devel\nXS-Autobuild: yes\n",
            ),
            // The area must be followed by a section.
            group(SOURCE_UPLOAD, "Section: non-free\n"),
        ];
        let fail = vec![
            group(SOURCE_UPLOAD, "Section: non-free/devel\n"),
            group(SOURCE_UPLOAD, "Section: non-free-firmware/kernel\n"),
            group(SOURCE_UPLOAD, "Section: multiverse/devel\n"),
            group(SOURCE_UPLOAD, "Section: restricted/devel\n"),
            group(SOURCE_UPLOAD, "Section: non-free/devel\nXS-Autobuild: no\n"),
        ];
        GroupTester::new(SourceOnlyUploadToNonFreeWithoutAutobuild, pass, fail).test_and_snapshot();
    }
}
