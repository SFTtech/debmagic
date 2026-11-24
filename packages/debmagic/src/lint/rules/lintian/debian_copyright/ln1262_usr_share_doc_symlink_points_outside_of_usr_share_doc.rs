use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct UsrShareDocSymlinkPointsOutsideOfUsrShareDoc;

declare_rule! {
    /// `usr/share/doc/<package>` is a symbolic link whose target is an
    /// absolute path or starts with `../`, so it points outside of
    /// `/usr/share/doc` (debian-policy 12.5).
    ///
    /// The link is read as stored and not resolved. Only the exact
    /// `usr/share/doc/<package>` entry is checked, not a directory with a
    /// trailing slash. Udebs are skipped (no copyright in udebs), as in
    /// `usr-share-doc-symlink-without-dependency`. Extra is the link target
    /// text; the Location is the symlink. Error severity, from the `.tag` file.
    UsrShareDocSymlinkPointsOutsideOfUsrShareDoc,
    code = "LN1262",
    tag = "usr-share-doc-symlink-points-outside-of-usr-share-doc",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for UsrShareDocSymlinkPointsOutsideOfUsrShareDoc {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let package = ctx.package_name().to_string();
        let Some(files) = ctx.data_files() else {
            return;
        };
        let Some(doclink) = files.get(format!("usr/share/doc/{package}")) else {
            return;
        };
        if doclink.kind() != EntryKind::Symlink {
            return;
        }
        let Some(link) = doclink.link_target() else {
            return;
        };
        let link = link.to_string_lossy().into_owned();
        if !(link.starts_with('/') || link.starts_with("../")) {
            return;
        }
        let location = Location {
            path: doclink.path().to_path_buf(),
            line: None,
            column: None,
        };
        ctx.diagnostic(Diagnostic::error(link).with_location(location));
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn link(target: &str) -> TestFile {
        TestFile::symlink(target)
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            // Not a symbolic link.
            BinaryPackageCase::from(vec![("usr/share/doc/example", TestFile::directory())]),
            // Relative targets that stay below /usr/share/doc.
            BinaryPackageCase::from(vec![("usr/share/doc/example", link("foreign-package"))]),
            BinaryPackageCase::from(vec![("usr/share/doc/example", link("foreign-package/doc"))]),
            // A target that merely starts with dots is not `../`.
            BinaryPackageCase::from(vec![("usr/share/doc/example", link("..foreign"))]),
            // A udeb has no copyright.
            BinaryPackageCase::from(vec![("usr/share/doc/example", link("/usr/share/doc/x"))])
                .udeb(),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example",
                link("/usr/share/doc/foreign"),
            )]),
            BinaryPackageCase::from(vec![("usr/share/doc/example", link("../foreign"))]),
            BinaryPackageCase::from(vec![("usr/share/doc/example", link("../../share/doc/x"))]),
        ];
        BinaryPackageTester::new(UsrShareDocSymlinkPointsOutsideOfUsrShareDoc, pass, fail)
            .test_and_snapshot();
    }
}
