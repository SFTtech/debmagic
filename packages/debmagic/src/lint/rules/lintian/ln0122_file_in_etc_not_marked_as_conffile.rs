use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct FileInEtcNotMarkedAsConffile;

declare_rule! {
    /// A regular file under `etc/` is not listed in `conffiles`.
    ///
    /// `etc/**/README` and `etc/init.d/skeleton`, `rc`, and `rcS` are exempt.
    /// Udebs are not checked. Extra is empty; the Data files path is the Location.
    FileInEtcNotMarkedAsConffile,
    code = "LN0122",
    tag = "file-in-etc-not-marked-as-conffile",
    default_selected = true,
    experimental = false,
}

static EXEMPT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(/README$|^etc/init\.d/(?:skeleton|rcS?)$)").expect("valid regex")
});

impl BinaryPackageRule for FileInEtcNotMarkedAsConffile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let declared = conffiles(ctx);
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink))
            .filter_map(|entry| {
                let path = entry.path().to_str()?;
                if !path.starts_with("etc/") || EXEMPT.is_match(path) || declared.contains(path) {
                    return None;
                }
                Some(PathBuf::from(path))
            })
            .collect();
        for path in paths {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

fn conffiles(ctx: &mut BinaryPackageContext<'_>) -> HashSet<String> {
    let Some(files) = ctx.control_files() else {
        return HashSet::new();
    };
    let Some(entry) = files.get("conffiles") else {
        return HashSet::new();
    };
    let Ok(bytes) = files.read(entry) else {
        return HashSet::new();
    };
    let Ok(text) = String::from_utf8(bytes) else {
        return HashSet::new();
    };
    text.lines()
        .filter_map(|line| {
            let path = line.split_whitespace().next()?;
            let path = path.strip_prefix('/').unwrap_or(path);
            (!path.is_empty()).then(|| path.to_string())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn files(paths: &[&str]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            paths
                .iter()
                .map(|path| ((*path).to_string(), TestFile::new("x")))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            files(&[]),
            files(&["etc/foo.conf"]).control_file("conffiles", TestFile::new("/etc/foo.conf\n")),
            files(&["etc/init.d/skeleton", "etc/init.d/rc", "etc/init.d/rcS"]),
            files(&["etc/foo/README"]),
            files(&["usr/bin/foo"]),
            files(&["etc/foo.conf"]).udeb(),
        ];
        let fail = vec![
            files(&["etc/something.conf"]),
            files(&["etc/gconf/schemas/test.schema"]),
            files(&["etc/foo.conf"]).control_file("conffiles", TestFile::new("/etc/other.conf\n")),
        ];
        BinaryPackageTester::new(FileInEtcNotMarkedAsConffile, pass, fail).test_and_snapshot();
    }
}
