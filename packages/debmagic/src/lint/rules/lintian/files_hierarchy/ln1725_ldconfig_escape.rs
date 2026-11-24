use std::path::{Path, PathBuf};

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::{BinaryPackageRule, Severity};
use crate::lint::rules::lintian::fields_relations_more::architectures::is_ldconfig_folder;

use super::ln1563_architecture_escape::normalize;

pub struct LdconfigEscape;

declare_rule! {
    /// A symlink in an ld.so load path points to a sub-directory outside the
    /// load path.
    ///
    /// Walks the Data files symbolic links whose directory is one of lintian's
    /// `ldconfig_folders` (the general library directories and each release
    /// architecture's triplet directory). The target is the link normalized
    /// against its directory, as lintian's `link_normalized` (absolute targets
    /// are taken as they are; a target that leaves the package root is
    /// skipped). It is a hit when the target's directory is not itself an
    /// ldconfig folder, so a link into a sub-directory such as `opt/myapp/` or
    /// into another package's triplet directory is reported. Extra is the
    /// normalized target; the link is the Location. Severity is info and the
    /// Tag is not selected by default, as in lintian.
    ///
    /// Hardlinks and regular files are not symlinks and are not checked. A
    /// link whose target is a general library directory in another triplet
    /// is `architecture-escape`, a separate Rule; a target in the same ld.so
    /// folder is never reported. Non-UTF-8 paths are skipped. Udebs are
    /// checked too.
    LdconfigEscape,
    code = "LN1725",
    tag = "ldconfig-escape",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for LdconfigEscape {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut hits: Vec<(PathBuf, String)> = Vec::new();
        for entry in files.query().kinds([EntryKind::Symlink]).into_iter() {
            let Some(origin) = entry.path().parent().and_then(Path::to_str) else {
                continue;
            };
            if !is_ldconfig_folder(origin) {
                continue;
            }
            let Some(link) = entry.link_target().and_then(Path::to_str) else {
                continue;
            };
            let Some(target) = normalize(origin, link) else {
                continue;
            };
            let target_dir = target.rsplit_once('/').map_or("", |(dir, _)| dir);
            if !is_ldconfig_folder(target_dir) {
                hits.push((entry.path().to_path_buf(), target));
            }
        }
        for (path, target) in hits {
            ctx.diagnostic(
                Diagnostic::warn(target)
                    .with_location(Location {
                        path,
                        line: None,
                        column: None,
                    })
                    .with_severity(Severity::Info),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn link(path: &str, target: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::symlink(target))])
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            // Target is a general library directory.
            link("usr/lib/liba.so", "/usr/lib/liba.so.1"),
            link("usr/lib/liba.so", "liba.so.1"),
            // Target is the triplet directory of the same origin.
            link(
                "usr/lib/x86_64-linux-gnu/liba.so",
                "../../lib/x86_64-linux-gnu/a.so",
            ),
            // Origin is not an ld.so folder: `opt` is not searched.
            link("opt/myapp/liba.so", "/opt/myapp/a-module.so"),
            // A target that leaves the package root is skipped.
            link("usr/lib/liba.so", "../../../../a.so"),
            // Hardlinks and regular files are not symlinks.
            BinaryPackageCase::from(vec![("usr/lib/liba.so", TestFile::new("x"))]),
            // A link at the package root has no ldconfig directory.
            link("liba.so", "/opt/myapp/a-module.so"),
        ];
        let fail = vec![
            link("usr/lib/myapp-module.so", "/opt/myapp/a-module.so"),
            // A relative link into a sub-directory.
            link("usr/lib/liba.so", "myapp/liba.so.1"),
            // A link to a sub-directory of another triplet's library directory.
            link(
                "lib/x86_64-linux-gnu/liba.so",
                "../../aarch64-linux-gnu/a.so",
            ),
            // A target at the top of the root has no directory, and is not an
            // ldconfig folder.
            link("usr/lib/liba.so", "/liba.so.1"),
            // `..` inside the link is normalized before the test.
            link("usr/lib/liba.so", "../lib/../../opt/a.so"),
            // Udebs are checked too.
            link("usr/lib/liba.so", "/opt/myapp/a-module.so").udeb(),
        ];
        BinaryPackageTester::new(LdconfigEscape, pass, fail).test_and_snapshot();
    }
}
