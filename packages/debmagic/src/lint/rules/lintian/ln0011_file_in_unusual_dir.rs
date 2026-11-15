use std::path::PathBuf;
use std::sync::LazyLock;

use regex::bytes::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct FileInUnusualDir;

declare_rule! {
    /// A file or symlink of the Binary package sits in a directory FHS and Policy do not expect.
    ///
    /// Walks Data files entries other than directories. A path is usual when
    /// it is under `usr/{bin,dict,doc,games,include,info,lib*,man,sbin,share,src,X11R6}/`,
    /// `lib/`, `lib32/`, `lib64/`, `libx32/`, `var/{games,lib,www,named}/`,
    /// `{bin,boot,dev,etc,sbin}/`, `usr/<triplet>-linux*/`, `usr/libexec/`,
    /// `usr/iraf/`, or under a location other Tags judge on their own
    /// (`build`, `home`, `mnt`, `opt`, `root`, `run`, `srv`, `tmp`, `usr/tmp`,
    /// `var/tmp`, and `var/www/` anywhere in the path). Udebs are skipped.
    /// Extra is empty; the Data files path is the Location.
    FileInUnusualDir,
    code = "LN0011",
    tag = "file-in-unusual-dir",
    default_selected = true,
    experimental = false,
}

static USUAL: LazyLock<Regex> = LazyLock::new(|| {
    // Bytes, not text: a name that is not UTF-8 is judged by its directory too.
    Regex::new(concat!(
        r"(?-u)^usr/(?:bin|dict|doc|games|include|info|lib(?:x?32|64)?|man|sbin|share|src|X11R6)/",
        r"|^lib(?:x?32|64)?/",
        r"|^var/(?:games|lib|www|named)/",
        r"|^(?:bin|boot|dev|etc|sbin)/",
        r"|^usr/[^/]+-linux[^/]*/",
        r"|^usr/libexec/",
        r"|^usr/iraf/",
        r"|\A(?:build|home|mnt|opt|root|run|srv|(?:(?:usr|var)/)?tmp)",
        r"|var/www/",
    ))
    .expect("valid regex")
});

impl BinaryPackageRule for FileInUnusualDir {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .query()
            .kinds([
                EntryKind::RegularFile,
                EntryKind::Hardlink,
                EntryKind::Symlink,
            ])
            .into_iter()
            .map(|entry| entry.path().to_path_buf())
            .filter(|path| !USUAL.is_match(path.as_os_str().as_encoded_bytes()))
            .collect();
        for path in paths {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn files(paths: &[&str]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            paths
                .iter()
                .map(|path| (path.to_string(), TestFile::new("content")))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            files(&[
                "usr/bin/example",
                "usr/sbin/example",
                "usr/share/doc/example/copyright",
                "usr/lib/example/lib.so",
                "usr/lib64/example.so",
                "usr/libx32/example.so",
                "usr/include/example.h",
                "usr/games/example",
                "usr/libexec/example",
                "usr/x86_64-linux-gnu/example",
                "usr/iraf/example",
                "usr/X11R6/bin/example",
                "lib/modules/example.ko",
                "lib32/example.so",
                "bin/example",
                "sbin/example",
                "boot/example",
                "dev/example",
                "etc/example.conf",
                "var/lib/example/state",
                "var/games/score",
                "var/www/index.html",
                "var/named/zone",
            ]),
            // Judged by other Tags.
            files(&[
                "opt/example/bin",
                "home/user/file",
                "mnt/file",
                "root/file",
                "run/example.pid",
                "srv/example",
                "tmp/file",
                "usr/tmp/file",
                "var/tmp/file",
                "build/file",
                "var/lib/foo/../x",
            ]),
            // `var/www/` anywhere in the path is accepted.
            files(&["other/var/www/index.html"]),
            // Directories are not files.
            BinaryPackageCase::from(vec![
                ("weird", TestFile::directory()),
                ("usr/weird", TestFile::directory()),
            ]),
            files(&["weird/file"]).udeb(),
            // A name that is not UTF-8 is judged by its directory.
            BinaryPackageCase::from(vec![(
                OsString::from_vec(b"usr/share/foo/bokm\xe5l".to_vec()),
                TestFile::new("content"),
            )]),
        ];
        let fail = vec![
            files(&["weird"]),
            files(&["weird/file"]),
            files(&["usr/weird/file"]),
            files(&["usr/lib"]),
            files(&["usr/etc/example.conf"]),
            files(&["var/cache/example"]),
            files(&["var/log/example"]),
            files(&["usr/local/bin/example"]),
            files(&["usr/x86_64/example"]),
            files(&["libfoo/example"]),
            BinaryPackageCase::from(vec![("weird/link", TestFile::symlink("/usr/bin/true"))]),
            files(&[
                "usr/bin-extra/file",
                "usr/share/doc/example/copyright",
                "stray",
            ]),
            BinaryPackageCase::from(vec![(
                OsString::from_vec(b"files/bokm\xe5l".to_vec()),
                TestFile::new("content"),
            )]),
        ];
        BinaryPackageTester::new(FileInUnusualDir, pass, fail).test_and_snapshot();
    }
}
