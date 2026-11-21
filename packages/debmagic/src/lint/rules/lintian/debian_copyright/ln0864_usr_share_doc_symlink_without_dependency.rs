use std::sync::LazyLock;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::file_tree::EntryKind;
use crate::lint::package_relation::PackageRelation;
use crate::lint::rule::BinaryPackageRule;

pub struct UsrShareDocSymlinkWithoutDependency;

declare_rule! {
    /// `usr/share/doc/<package>` is a symbolic link to another package's
    /// documentation directory, and the package does not depend on that
    /// package.
    ///
    /// The link is read as stored. A link that is absolute or starts with
    /// `../` is not this Rule (lintian reports those as
    /// `usr-share-doc-symlink-points-outside-of-usr-share-doc`). Otherwise the
    /// name is the link text up to its first `/`, and a hit is any such name
    /// that the Binary package does not depend on: not in its `Pre-Depends` or
    /// `Depends` (the strong relations, AND-ed), not implied with its
    /// architecture qualifier when the Architecture is not `all`, and not an
    /// essential package (`data/fields/essential`). Transitive dependencies do
    /// not count. Udebs are skipped (no copyright in udebs). Extra is the
    /// package name; there is no Location, as the Subject is the Binary
    /// package. Error severity, from the `.tag` file.
    UsrShareDocSymlinkWithoutDependency,
    code = "LN0864",
    tag = "usr-share-doc-symlink-without-dependency",
    default_selected = true,
    experimental = false,
}

/// lintian 2.144.0 `data/fields/essential`, vendored in `src/lint/data`.
static ESSENTIAL: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    crate::lint::data::lines(include_str!("../../../data/fields/essential")).collect()
});

impl BinaryPackageRule for UsrShareDocSymlinkWithoutDependency {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let package = ctx.package_name().to_string();
        let (strong, architecture) = {
            let fields = ctx.installation_control().fields();
            (
                fields.relations_and(&["Pre-Depends", "Depends"]),
                fields
                    .get("Architecture")
                    .unwrap_or_default()
                    .trim()
                    .to_string(),
            )
        };
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
        if link.starts_with('/') || link.starts_with("../") {
            return;
        }
        let target = link.split('/').next().unwrap_or_default().to_string();
        if depends_on(&strong, &architecture, &target) {
            return;
        }
        ctx.diagnostic(Diagnostic::error(target));
    }
}

/// lintian's `depends_on`: an essential package, or a strong relation that
/// implies the name (or its `:<arch>` form when the Architecture is not `all`).
fn depends_on(strong: &PackageRelation, architecture: &str, package: &str) -> bool {
    if ESSENTIAL.contains(&package) {
        return true;
    }
    if strong.satisfies(package) {
        return true;
    }
    architecture != "all" && strong.satisfies(&format!("{package}:{architecture}"))
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
            BinaryPackageCase::from(vec![("usr/share/doc/example", TestFile::directory())])
                .field("Architecture", "all"),
            // Depends (Pre-Depends too), with or without a version.
            BinaryPackageCase::from(vec![("usr/share/doc/example", link("foreign-package/"))])
                .field("Architecture", "all")
                .field("Depends", "foreign-package (>= 1)"),
            BinaryPackageCase::from(vec![("usr/share/doc/example", link("foreign-package/sub"))])
                .field("Architecture", "all")
                .field("Pre-Depends", "foreign-package"),
            // The architecture-qualified form.
            BinaryPackageCase::from(vec![("usr/share/doc/example", link("foreign-package"))])
                .field("Architecture", "amd64")
                .field("Depends", "foreign-package:amd64"),
            // Essential packages need no dependency.
            BinaryPackageCase::from(vec![("usr/share/doc/example", link("base-files"))])
                .field("Architecture", "all"),
            // Absolute and ../ links are the points-outside Tag, not this one.
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example",
                link("/usr/share/doc/foreign"),
            )])
            .field("Architecture", "all"),
            BinaryPackageCase::from(vec![("usr/share/doc/example", link("../foreign"))])
                .field("Architecture", "all"),
            // A udeb has no copyright.
            BinaryPackageCase::from(vec![("usr/share/doc/example", link("foreign-package"))])
                .field("Architecture", "all")
                .udeb(),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![("usr/share/doc/example", link("foreign-package"))])
                .field("Architecture", "all"),
            // Only the name before the first slash counts.
            BinaryPackageCase::from(vec![("usr/share/doc/example", link("foreign-package/doc"))])
                .field("Architecture", "amd64"),
            // Weak dependencies do not count.
            BinaryPackageCase::from(vec![("usr/share/doc/example", link("foreign-package"))])
                .field("Architecture", "all")
                .field("Recommends", "foreign-package"),
            // Another architecture's qualifier does not satisfy the name.
            BinaryPackageCase::from(vec![("usr/share/doc/example", link("foreign-package"))])
                .field("Architecture", "amd64")
                .field("Depends", "foreign-package:arm64"),
        ];
        BinaryPackageTester::new(UsrShareDocSymlinkWithoutDependency, pass, fail)
            .test_and_snapshot();
    }
}
