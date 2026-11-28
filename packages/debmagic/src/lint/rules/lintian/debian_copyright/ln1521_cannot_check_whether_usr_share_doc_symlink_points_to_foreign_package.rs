use crate::declare_rule;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::file_tree::EntryKind;
use crate::lint::group_context::GroupContext;
use crate::lint::rule::{GroupRule, Severity};

use super::ln0864_usr_share_doc_symlink_without_dependency::depends_on;

pub struct CannotCheckWhetherUsrShareDocSymlinkPointsToForeignPackage;

declare_rule! {
    /// `usr/share/doc/<package>` is a symbolic link to the documentation of
    /// another package, which this Binary package depends on, and the Group
    /// has no `.dsc` to say whether that package comes from the same source.
    /// The Diagnostic has no extra and attaches to the installable with the
    /// symlink. Info severity, from the `.tag` file.
    ///
    /// The link is looked at as `usr-share-doc-symlink-to-foreign-package`
    /// looks at it: only a link that is neither absolute nor starts with `../`,
    /// its name the link text up to the first `/`, and only when the installable
    /// depends on that name (the same test as
    /// `usr-share-doc-symlink-without-dependency`). A name that is an installable
    /// of the Group is fine (the Group is one source). Udebs are skipped.
    ///
    /// A Group Rule that runs on a Group without a `.dsc` (lintian's per-binary
    /// check, which gets the source only when there is one). With a
    /// `.dsc` this Rule is silent: `usr-share-doc-symlink-to-foreign-package`
    /// decides from the source's `debian/control` instead.
    CannotCheckWhetherUsrShareDocSymlinkPointsToForeignPackage,
    code = "LN1521",
    tag = "cannot-check-whether-usr-share-doc-symlink-points-to-foreign-package",
    default_selected = false,
    experimental = false,
}

impl GroupRule for CannotCheckWhetherUsrShareDocSymlinkPointsToForeignPackage {
    fn run(&self, ctx: &mut GroupContext<'_>) {
        if ctx.dsc().is_some() {
            return;
        }
        let installables = ctx.installables();
        for installable in &installables {
            if installable.is_udeb() {
                continue;
            }
            let Some(files) = ctx.data_files(installable) else {
                continue;
            };
            let Some(doclink) = files.get(format!("usr/share/doc/{}", installable.name())) else {
                continue;
            };
            if doclink.kind() != EntryKind::Symlink {
                continue;
            }
            let Some(link) = doclink.link_target() else {
                continue;
            };
            let link = link.to_string_lossy().into_owned();
            if link.starts_with('/') || link.starts_with("../") {
                continue;
            }
            let foreign = link.split('/').next().unwrap_or_default();
            if !depends_on(
                &installable.strong_relation(),
                installable.architecture(),
                foreign,
            ) {
                continue;
            }
            if installables.iter().any(|sibling| sibling.name() == foreign) {
                continue;
            }
            ctx.installable_diagnostic(
                installable,
                Diagnostic::warn("").with_severity(Severity::Info),
            );
        }
    }

    fn runs_without_dsc(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{GroupCase, GroupTester, TestFile};

    use super::*;

    const DSC: &str = "Source: example\nVersion: 1.0\n";

    /// A lone `.deb` named `example`, with the given fields and a
    /// `usr/share/doc/example` symlink to `link`.
    fn lone(fields: &str, link: &str) -> GroupCase {
        GroupCase::without_dsc()
            .installable("example", fields)
            .files([("usr/share/doc/example", TestFile::symlink(link))])
    }

    #[test]
    fn group() {
        let pass = vec![
            // Without the dependency, the other Tag speaks.
            lone("", "foreign-package"),
            // Absolute and ../ links, no link at all.
            lone(
                "Depends: foreign-package\n",
                "/usr/share/doc/foreign-package",
            ),
            lone("Depends: foreign-package\n", "../foreign-package"),
            GroupCase::without_dsc()
                .installable("example", "Depends: foreign-package\n")
                .files([("usr/share/doc/example", TestFile::directory())]),
            // A udeb.
            GroupCase::without_dsc()
                .udeb("example", "Depends: foreign-package\n")
                .files([(
                    "usr/share/doc/example",
                    TestFile::symlink("foreign-package"),
                )]),
            // A sibling of the Group names the package: one source by construction.
            GroupCase::without_dsc()
                .installable("example", "Depends: sibling\n")
                .files([("usr/share/doc/example", TestFile::symlink("sibling"))])
                .installable("sibling", ""),
            // With a `.dsc`, usr-share-doc-symlink-to-foreign-package decides.
            GroupCase::new(DSC)
                .installable("example", "Depends: foreign-package\n")
                .files([(
                    "usr/share/doc/example",
                    TestFile::symlink("foreign-package"),
                )]),
        ];
        let fail = vec![
            // A lone .deb depending on the package it links into.
            lone("Depends: foreign-package\n", "foreign-package"),
            lone(
                "Pre-Depends: foreign-package (>= 1)\n",
                "foreign-package/sub/dir",
            ),
            // A sibling in the Group does not make the foreign name a sibling.
            GroupCase::without_dsc()
                .installable("example", "Depends: foreign-package\n")
                .files([(
                    "usr/share/doc/example",
                    TestFile::symlink("foreign-package"),
                )])
                .installable("sibling", ""),
            // An essential package is depended on without saying so.
            lone("", "base-files"),
        ];
        GroupTester::new(
            CannotCheckWhetherUsrShareDocSymlinkPointsToForeignPackage,
            pass,
            fail,
        )
        .test_and_snapshot();
    }
}
