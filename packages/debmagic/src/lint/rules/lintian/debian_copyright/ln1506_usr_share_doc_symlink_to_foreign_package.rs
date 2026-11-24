use crate::declare_rule;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::file_tree::EntryKind;
use crate::lint::group_context::GroupContext;
use crate::lint::rule::GroupRule;

use super::ln0864_usr_share_doc_symlink_without_dependency::depends_on;

pub struct UsrShareDocSymlinkToForeignPackage;

declare_rule! {
    /// `usr/share/doc/<package>` is a symbolic link to the documentation of a
    /// package that the source package does not build. The extra is the
    /// package name, the link text up to its first `/`.
    ///
    /// Only a link that is neither absolute nor starts with `../` is looked
    /// at, and only when the package depends on the package it names (an
    /// essential package, a Pre-Depends or Depends, also with its
    /// architecture qualifier: the same test as for
    /// `usr-share-doc-symlink-without-dependency`). The source package's
    /// installables are those of its `debian/control`. udebs are skipped.
    ///
    /// A Group Rule: it needs the Group's `.dsc` and its `debian/control`
    /// (lintian says `cannot-check-whether-usr-share-doc-symlink-points-to-foreign-package`
    /// when the source is missing, which this Rule does not report), so it runs
    /// once per Group that has a `.dsc`. The Diagnostic attaches to the
    /// installable with the symlink, as in lintian.
    UsrShareDocSymlinkToForeignPackage,
    code = "LN1506",
    tag = "usr-share-doc-symlink-to-foreign-package",
    default_selected = true,
    experimental = false,
}

impl GroupRule for UsrShareDocSymlinkToForeignPackage {
    fn run(&self, ctx: &mut GroupContext<'_>) {
        let Some(control) = ctx.debian_control().cloned() else {
            return;
        };
        for installable in ctx.installables() {
            if installable.is_udeb() {
                continue;
            }
            let Some(files) = ctx.data_files(&installable) else {
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
            if control
                .installables()
                .iter()
                .any(|declared| declared.name() == foreign)
            {
                continue;
            }
            ctx.installable_diagnostic(&installable, Diagnostic::error(foreign));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{GroupCase, GroupTester, TestFile};

    use super::*;

    const DSC: &str = "Source: example\nVersion: 1.0\n";
    const CONTROL: &str = "Source: example\n\nPackage: example\nArchitecture: all\nDescription: x\n x\n\nPackage: sibling\nArchitecture: all\nDescription: x\n x\n";

    fn group(fields: &str, link: &str) -> GroupCase {
        GroupCase::new(DSC)
            .source_files([("debian/control", CONTROL)])
            .installable("example", fields)
            .files([("usr/share/doc/example", TestFile::symlink(link))])
    }

    #[test]
    fn group_rule() {
        let pass = vec![
            // A package of the source.
            group("Depends: sibling\n", "sibling"),
            group("Depends: sibling\n", "sibling/sub"),
            // Without the dependency, the other Tag speaks.
            group("", "foreign-package"),
            // Absolute and ../ links, no link at all.
            group(
                "Depends: foreign-package\n",
                "/usr/share/doc/foreign-package",
            ),
            group("Depends: foreign-package\n", "../foreign-package"),
            GroupCase::new(DSC)
                .source_files([("debian/control", CONTROL)])
                .installable("example", "Depends: foreign-package\n")
                .files([("usr/share/doc/example", TestFile::directory())]),
            // A udeb.
            GroupCase::new(DSC)
                .source_files([("debian/control", CONTROL)])
                .udeb("example", "Depends: foreign-package\n")
                .files([(
                    "usr/share/doc/example",
                    TestFile::symlink("foreign-package"),
                )]),
        ];
        let fail = vec![
            group("Depends: foreign-package\n", "foreign-package"),
            group(
                "Pre-Depends: foreign-package (>= 1)\n",
                "foreign-package/sub/dir",
            ),
            // An essential package is depended on without saying so.
            group("", "base-files"),
            GroupCase::new(DSC)
                .source_files([("debian/control", CONTROL)])
                .installable("example", "Depends: a\n")
                .files([("usr/share/doc/example", TestFile::symlink("a"))])
                .installable("sibling", "Depends: b\n")
                .files([("usr/share/doc/sibling", TestFile::symlink("b"))]),
        ];
        GroupTester::new(UsrShareDocSymlinkToForeignPackage, pass, fail).test_and_snapshot();
    }
}
