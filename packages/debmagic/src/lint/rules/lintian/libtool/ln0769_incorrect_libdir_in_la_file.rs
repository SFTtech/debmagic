use std::path::{Path, PathBuf};

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::rule::BinaryPackageRule;

pub struct IncorrectLibdirInLaFile;

declare_rule! {
    /// A libtool `.la` file names a `libdir` other than the directory it is
    /// installed in.
    ///
    /// Walks the Data files that are regular files named `*.la` with valid
    /// UTF-8 content (symbolic links and hardlinks are skipped). Each
    /// `libdir='...'` line is compared, without leading slashes and with one
    /// trailing slash, to the file's directory (with trailing slash). A
    /// mismatch is reported at the line, with `<libdir> != <dirname>` as the
    /// extra. python-central files in `usr/share/pyshared/` may name a
    /// `usr/lib/python<version>/(site|dist)-packages/` libdir. Udebs are
    /// checked too.
    IncorrectLibdirInLaFile,
    code = "LN0769",
    tag = "incorrect-libdir-in-la-file",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for IncorrectLibdirInLaFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut found = Vec::new();
        for (path, text) in la_files(files) {
            let dirname = dirname(&path);
            for (index, line) in text.split('\n').enumerate() {
                let Some(libdir) = la_value(line, "libdir") else {
                    continue;
                };
                let own = format!("{}/", libdir.trim_start_matches('/').trim_end_matches('/'));
                if own == dirname || is_python_central(&own, &dirname) {
                    continue;
                }
                found.push((
                    path.clone(),
                    index as u32 + 1,
                    format!("{own} != {dirname}"),
                ));
            }
        }
        for (path, line, extra) in found {
            ctx.diagnostic(Diagnostic::error(extra).with_location(Location {
                path,
                line: Some(line),
                column: None,
            }));
        }
    }
}

/// Regular `.la` files with valid UTF-8 content, as `(path, text)`. Like
/// lintian, a symbolic link or hardlink (its `link` is set) is not read.
pub(super) fn la_files(files: &FileTree) -> Vec<(PathBuf, String)> {
    files
        .query()
        .kinds([EntryKind::RegularFile])
        .into_iter()
        .filter(|entry| {
            entry
                .path()
                .to_str()
                .is_some_and(|name| name.ends_with(".la"))
        })
        .filter_map(|entry| {
            let text = String::from_utf8(files.read(entry).ok()?).ok()?;
            Some((entry.path().to_path_buf(), text))
        })
        .collect()
}

/// The value of a `key='value'` line; `None` for other lines and empty values.
pub(super) fn la_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    line.strip_prefix(key)?
        .strip_prefix("='")?
        .strip_suffix('\'')
        .filter(|value| !value.is_empty())
}

/// The directory of `path` with a trailing slash, or empty at top level.
fn dirname(path: &Path) -> String {
    match path.parent().and_then(|dir| dir.to_str()) {
        Some(dir) if !dir.is_empty() => format!("{dir}/"),
        _ => String::new(),
    }
}

/// python-central installs `usr/share/pyshared/` files under
/// `usr/lib/python<version>/(site|dist)-packages/` at install time.
fn is_python_central(own: &str, dirname: &str) -> bool {
    if !dirname.starts_with("usr/share/pyshared/") {
        return false;
    }
    let Some(rest) = own.strip_prefix("usr/lib/python") else {
        return false;
    };
    let version = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(rest.len());
    let tail = &rest[version..];
    version > 0 && (tail.starts_with("/site-packages/") || tail.starts_with("/dist-packages/"))
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn la(text: &str) -> TestFile {
        TestFile::new(text)
    }

    #[test]
    fn python_central_libdirs() {
        assert!(is_python_central(
            "usr/lib/python3.12/site-packages/",
            "usr/share/pyshared/foo/"
        ));
        assert!(is_python_central(
            "usr/lib/python2.7/dist-packages/",
            "usr/share/pyshared/"
        ));
        assert!(!is_python_central(
            "usr/lib/python3.12/site-packages/",
            "usr/lib/foo/"
        ));
        assert!(!is_python_central(
            "usr/lib/python3.12/foo/",
            "usr/share/pyshared/"
        ));
        assert!(!is_python_central(
            "usr/lib/python/site-packages/",
            "usr/share/pyshared/"
        ));
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            // Matching libdir, with or without the slashes.
            BinaryPackageCase::from(vec![(
                "usr/lib/x86_64-linux-gnu/libfoo.la",
                la("libdir='/usr/lib/x86_64-linux-gnu'\n"),
            )])
            .field("Architecture", "amd64"),
            BinaryPackageCase::from(vec![("usr/lib/x/a.la", la("libdir='usr/lib/x/'\n"))])
                .field("Architecture", "amd64"),
            // Empty libdir and other lines are not judged.
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.la",
                la("libdir=''\nlibdir=/elsewhere\n libdir='/elsewhere'\n"),
            )])
            .field("Architecture", "amd64"),
            // python-central files may name the installed python directory.
            BinaryPackageCase::from(vec![(
                "usr/share/pyshared/foo/libbar.la",
                la("libdir='/usr/lib/python3.12/site-packages'\n"),
            )])
            .field("Architecture", "amd64"),
            // Only `.la` regular files are read.
            BinaryPackageCase::from(vec![("usr/lib/libfoo.a", la("libdir='/elsewhere'\n"))])
                .field("Architecture", "amd64"),
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.la",
                TestFile::symlink("/elsewhere/libfoo.la"),
            )])
            .field("Architecture", "amd64"),
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.la",
                la("libdir='/elsewhere'\n").hardlink(),
            )])
            .field("Architecture", "amd64"),
            // Invalid UTF-8 is not decoded by lintian.
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.la",
                TestFile::from_bytes(b"libdir='/elsewhere'\n\xff".to_vec()),
            )])
            .field("Architecture", "amd64"),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![(
                "usr/lib/x86_64-linux-gnu/libfoo.la",
                la("libdir='/usr/lib/other'\n"),
            )])
            .field("Architecture", "amd64"),
            // Reported at the line of the libdir.
            BinaryPackageCase::from(vec![(
                "usr/lib/kde3/kio.la",
                la("# libtool\n\nlibdir='/build/usr/lib/kde3'\n"),
            )])
            .field("Architecture", "amd64"),
            // Two libdir lines are two reports.
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.la",
                la("libdir='/a'\nlibdir='/b'\n"),
            )])
            .field("Architecture", "amd64"),
            // A top-level file has an empty dirname.
            BinaryPackageCase::from(vec![("libfoo.la", la("libdir='/usr/lib'\n"))])
                .field("Architecture", "amd64"),
            // python-central only covers the python directories.
            BinaryPackageCase::from(vec![(
                "usr/share/pyshared/foo/libbar.la",
                la("libdir='/usr/lib/python3.12/foo'\n"),
            )])
            .field("Architecture", "amd64"),
            BinaryPackageCase::from(vec![(
                "usr/lib/python3/dist-packages/libbar.la",
                la("libdir='/usr/lib/python3/site-packages'\n"),
            )])
            .field("Architecture", "amd64"),
            // Udebs are checked too.
            BinaryPackageCase::from(vec![("usr/lib/libfoo.la", la("libdir='/elsewhere'\n"))])
                .field("Architecture", "amd64")
                .udeb(),
        ];
        BinaryPackageTester::new(IncorrectLibdirInLaFile, pass, fail).test_and_snapshot();
    }
}
