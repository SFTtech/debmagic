use std::path::{Path, PathBuf};

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::{BinaryPackageRule, Severity};
use crate::lint::rules::lintian::fields_relations_more::architectures::{
    deb_host_multiarch_triplets, is_ldconfig_folder,
};

pub struct ArchitectureEscape;

declare_rule! {
    /// A symlink in an architecture-specific library directory points to a
    /// general one: `usr/lib/<triplet>/liba.so` to `usr/lib/liba.so`, or
    /// `lib/<triplet>/liba.so` to `lib/liba.so`.
    ///
    /// Walks the Data files symbolic links whose directory is one of lintian's
    /// `ldconfig_folders` (any release architecture's triplet directory under
    /// `lib/` or `usr/lib/`, or a general library directory) and whose last
    /// component is a `DEB_HOST_MULTIARCH` triplet. The target is the link
    /// normalized against its directory, as lintian's `link_normalized`
    /// (absolute targets are taken as they are; a target that leaves the
    /// package root is skipped). It is a hit when the target's directory is
    /// exactly `lib`, `usr/lib` or `usr/local/lib`. Extra is the normalized
    /// target; the link is the Location. Severity is info and the Tag is not
    /// selected by default, as in lintian.
    ///
    /// Hardlinks are not symlinks and are not checked. Non-UTF-8 paths are
    /// skipped. The `ldconfig-escape` Tag, for links into a sub-directory, is
    /// a separate Rule.
    ArchitectureEscape,
    code = "LN1563",
    tag = "architecture-escape",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for ArchitectureEscape {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let triplets = deb_host_multiarch_triplets();
        let mut hits: Vec<(PathBuf, String)> = Vec::new();
        for entry in files.query().kinds([EntryKind::Symlink]).into_iter() {
            let Some(origin) = entry.path().parent().and_then(Path::to_str) else {
                continue;
            };
            if !is_ldconfig_folder(origin) {
                continue;
            }
            let Some(origin_name) = origin.rsplit('/').next() else {
                continue;
            };
            if !triplets.contains(&origin_name) {
                continue;
            }
            let Some(link) = entry.link_target().and_then(Path::to_str) else {
                continue;
            };
            let Some(target) = normalize(origin, link) else {
                continue;
            };
            let target_dir = target.rsplit_once('/').map_or("", |(dir, _)| dir);
            if matches!(target_dir, "lib" | "usr/lib" | "usr/local/lib") {
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

/// `normalize_link_target`: the link against `dir` (or taken as is when
/// absolute), with `.` dropped and `..` popped. `None` when it leaves the root.
fn normalize(dir: &str, link: &str) -> Option<String> {
    let joined = if link.starts_with('/') {
        link.to_string()
    } else {
        format!("{dir}/{link}")
    };
    let mut parts: Vec<&str> = Vec::new();
    for part in joined.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            _ => parts.push(part),
        }
    }
    Some(parts.join("/"))
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
            // Target stays in the triplet directory.
            link("usr/lib/x86_64-linux-gnu/liba.so", "liba.so.1"),
            // Origin is not a triplet directory.
            link("usr/lib/liba.so", "/usr/lib/a.so"),
            // Target is not a general library directory.
            link(
                "usr/lib/x86_64-linux-gnu/liba.so",
                "../x86_64-linux-gnu/a.so",
            ),
            link("usr/lib/x86_64-linux-gnu/liba.so", "/usr/share/a.so"),
            // A target that leaves the package root is skipped.
            link("usr/lib/x86_64-linux-gnu/liba.so", "../../../../a.so"),
            // Hardlinks and regular files are not symlinks.
            BinaryPackageCase::from(vec![(
                "usr/lib/x86_64-linux-gnu/liba.so",
                TestFile::new("x"),
            )]),
            // A link at the package root has no ldconfig directory.
            link("liba.so", "/usr/lib/a.so"),
        ];
        let fail = vec![
            link("usr/lib/x86_64-linux-gnu/liba.so", "/usr/lib/a.so"),
            link("usr/lib/x86_64-linux-gnu/liba.so", "../a.so"),
            link("lib/aarch64-linux-gnu/liba.so", "../liba.so"),
            link("usr/lib/aarch64-linux-gnu/liba.so", "../../local/lib/a.so"),
            // `..` inside the link is normalized before the test.
            link("usr/lib/x86_64-linux-gnu/liba.so", "../../lib/../lib/a.so"),
        ];
        BinaryPackageTester::new(ArchitectureEscape, pass, fail).test_and_snapshot();
    }
}
