use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;
use crate::lint::rules::lintian::fields_relations_more::architectures::deb_host_multiarch_triplets;

pub struct PkgConfigMultiArchWrongDir;

declare_rule! {
    /// A pkg-config file names the multiarch directory of another architecture.
    ///
    /// Walks Data files that are regular files (not hardlinks or symlinks)
    /// named `usr/lib/pkgconfig/<name>.pc`, `usr/lib/<dir>/pkgconfig/<name>.pc`
    /// or `usr/share/pkgconfig/<name>.pc`, for any Architecture, udebs too.
    /// The text has `#` comments removed up to the end of the line and each
    /// `\` newline joined by a space. The first DEB_HOST_MULTIARCH triplet in
    /// sorted order that is not `<dir>` and occurs with no word character
    /// (`[A-Za-z0-9_]`) on either side is a hit. Extra is `full text contains
    /// architecture specific dir <triplet>`; the file is the Location.
    PkgConfigMultiArchWrongDir,
    code = "LN0732",
    tag = "pkg-config-multi-arch-wrong-dir",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PkgConfigMultiArchWrongDir {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let triplets = deb_host_multiarch_triplets();
        let hits: Vec<(PathBuf, &str)> = files
            .query()
            .kinds([EntryKind::RegularFile])
            .into_iter()
            .filter_map(|entry| {
                let arch_dir = pkgconfig_arch_dir(entry.path().to_str()?)?;
                let text = normalized_text(&files.read(entry).ok()?);
                let triplet = triplets
                    .iter()
                    .copied()
                    .find(|triplet| *triplet != arch_dir && mentions(&text, triplet.as_bytes()))?;
                Some((entry.path().to_path_buf(), triplet))
            })
            .collect();
        for (path, triplet) in hits {
            ctx.diagnostic(
                Diagnostic::error(format!(
                    "full text contains architecture specific dir {triplet}"
                ))
                .with_location(Location {
                    path,
                    line: None,
                    column: None,
                }),
            );
        }
    }
}

static PKGCONFIG_PATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^usr/(?:lib(?:/([^/]+))?|share)/pkgconfig/[^/]+\.pc$").expect("valid regex")
});

/// The `<dir>` of a pkg-config path, empty when there is none; `None` when
/// the path is not a pkg-config file.
fn pkgconfig_arch_dir(name: &str) -> Option<&str> {
    let captures = PKGCONFIG_PATH.captures(name)?;
    Some(captures.get(1).map_or("", |dir| dir.as_str()))
}

/// The file text as the Check scans it: `#` comments removed up to, not
/// including, the end of the line, then each `\` newline joined by a space.
fn normalized_text(bytes: &[u8]) -> Vec<u8> {
    let mut uncommented = Vec::with_capacity(bytes.len());
    let mut in_comment = false;
    for &byte in bytes {
        if in_comment {
            in_comment = !matches!(byte, b'\n' | b'\r' | 0x0b | 0x0c);
            if in_comment {
                continue;
            }
        }
        if byte == b'#' {
            in_comment = true;
            continue;
        }
        uncommented.push(byte);
    }

    let mut joined = Vec::with_capacity(uncommented.len());
    let mut at = 0;
    while at < uncommented.len() {
        if uncommented[at] == b'\\' && uncommented.get(at + 1) == Some(&b'\n') {
            joined.push(b' ');
            at += 2;
        } else {
            joined.push(uncommented[at]);
            at += 1;
        }
    }
    joined
}

/// Whether `needle` occurs in `text` with no word character on either side.
/// The Check's `\W` needs a character before the match, so a match at the
/// very start of the text does not count.
fn mentions(text: &[u8], needle: &[u8]) -> bool {
    text.windows(needle.len()).enumerate().any(|(at, window)| {
        window == needle
            && at > 0
            && !is_word(text[at - 1])
            && text
                .get(at + needle.len())
                .is_none_or(|&byte| !is_word(byte))
    })
}

fn is_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn pc(text: &str) -> TestFile {
        TestFile::new(text)
    }

    #[test]
    fn pkgconfig_paths() {
        assert_eq!(pkgconfig_arch_dir("usr/lib/pkgconfig/libfoo.pc"), Some(""));
        assert_eq!(
            pkgconfig_arch_dir("usr/share/pkgconfig/libfoo.pc"),
            Some("")
        );
        assert_eq!(
            pkgconfig_arch_dir("usr/lib/x86_64-linux-gnu/pkgconfig/libfoo.pc"),
            Some("x86_64-linux-gnu")
        );
        assert_eq!(
            pkgconfig_arch_dir("usr/lib/x86_64-linux-gnu/pkgconfig/.pc"),
            None
        );
        assert_eq!(pkgconfig_arch_dir("usr/lib/pkgconfig/libfoo.pc.in"), None);
        assert_eq!(
            pkgconfig_arch_dir("usr/lib/x86_64-linux-gnu/pkgconfig/sub/libfoo.pc"),
            None
        );
        assert_eq!(
            pkgconfig_arch_dir("usr/lib/x86_64-linux-gnu/libfoo.pc"),
            None
        );
        assert_eq!(
            pkgconfig_arch_dir("usr/local/lib/pkgconfig/libfoo.pc"),
            None
        );
    }

    #[test]
    fn text_comments_and_continuations() {
        assert_eq!(
            normalized_text(b"Cflags: -I/a # b\nLibs: -c"),
            b"Cflags: -I/a \nLibs: -c"
        );
        // A comment ends at the newline, so a continuation after it is not joined.
        assert_eq!(normalized_text(b"# a \\\nb"), b"\nb");
        assert_eq!(
            normalized_text(b"Cflags: -I/a \\\n -I/b"),
            b"Cflags: -I/a   -I/b"
        );
    }

    #[test]
    fn mention_needs_word_boundaries() {
        let triplet = b"i386-linux-gnu";
        assert!(mentions(b"-L/usr/lib/i386-linux-gnu", triplet));
        assert!(mentions(b"-L/usr/lib/i386-linux-gnu/", triplet));
        assert!(mentions(b"a i386-linux-gnu", triplet));
        assert!(!mentions(b"i386-linux-gnu", triplet));
        assert!(!mentions(b"x_i386-linux-gnu", triplet));
        assert!(!mentions(b"xi386-linux-gnu", triplet));
        assert!(!mentions(b"i386-linux-gnu_x", triplet));
        assert!(!mentions(b"i386-linux-gnu2", triplet));
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(vec![(
                "usr/lib/pkgconfig/indep-good.pc",
                pc("Cflags: -I/usr/include/indep-good\n"),
            )]),
            // The file's own multiarch directory is not wrong.
            BinaryPackageCase::from(vec![(
                "usr/lib/x86_64-linux-gnu/pkgconfig/libfoo.pc",
                pc("Cflags: -I/usr/include/x86_64-linux-gnu\n"),
            )]),
            // Comments are not scanned.
            BinaryPackageCase::from(vec![(
                "usr/lib/pkgconfig/libfoo.pc",
                pc("# i386-linux-gnu\nName: foo\n"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/pkgconfig/libfoo.pc",
                pc("Cflags: -I/a # -I/usr/include/i386-linux-gnu\n"),
            )]),
            // Word characters beside the triplet: not a directory.
            BinaryPackageCase::from(vec![(
                "usr/lib/pkgconfig/libfoo.pc",
                pc("Libs: -Lfoo_i386-linux-gnu\n"),
            )]),
            // Nothing precedes the triplet at the start of the text.
            BinaryPackageCase::from(vec![(
                "usr/lib/pkgconfig/libfoo.pc",
                pc("i386-linux-gnu/lib\n"),
            )]),
            // Not a pkg-config file path.
            BinaryPackageCase::from(vec![(
                "usr/lib/pkgconfig/libfoo.pc.in",
                pc("Libs: -L/usr/lib/i386-linux-gnu\n"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/x86_64-linux-gnu/pkgconfig/sub/libfoo.pc",
                pc("Libs: -L/usr/lib/i386-linux-gnu\n"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.pc",
                pc("Libs: -L/usr/lib/i386-linux-gnu\n"),
            )]),
            // Only regular files are scanned.
            BinaryPackageCase::from(vec![(
                "usr/lib/pkgconfig/link.pc",
                TestFile::symlink("libfoo.pc"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/pkgconfig/hard.pc",
                pc("Libs: -L/usr/lib/i386-linux-gnu\n").hardlink(),
            )]),
        ];
        let fail = vec![
            // Architecture all is checked too.
            BinaryPackageCase::from(vec![(
                "usr/lib/pkgconfig/indep.pc",
                pc("Cflags: -I/usr/include/x86_64-linux-gnu\n"),
            )])
            .field("Architecture", "all"),
            BinaryPackageCase::from(vec![(
                "usr/lib/pkgconfig/indep.pc",
                pc("Cflags: -I/usr/include/x86_64-linux-gnu\n"),
            )])
            .field("Architecture", "amd64"),
            BinaryPackageCase::from(vec![(
                "usr/lib/x86_64-linux-gnu/pkgconfig/arch-cross.pc",
                pc("Cflags: -I/usr/include/i386-linux-gnu/someconfig.h\n"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/pkgconfig/foo.pc",
                pc("Libs: -L/usr/lib/i386-linux-gnu\n"),
            )]),
            // A triplet on a continuation line counts.
            BinaryPackageCase::from(vec![(
                "usr/lib/pkgconfig/foo.pc",
                pc("Cflags: -I/a \\\n  -I/usr/include/i386-linux-gnu\n"),
            )]),
            // A triplet at the very end of the text counts.
            BinaryPackageCase::from(vec![(
                "usr/lib/pkgconfig/foo.pc",
                pc("Libs: -L/usr/lib/i386-linux-gnu"),
            )]),
            // One hit per file: the sorted-first triplet that matches.
            BinaryPackageCase::from(vec![(
                "usr/lib/x86_64-linux-gnu/pkgconfig/foo.pc",
                pc("Libs: -L/usr/lib/i386-linux-gnu -L/usr/lib/aarch64-linux-gnu\n"),
            )]),
        ];
        BinaryPackageTester::new(PkgConfigMultiArchWrongDir, pass, fail).test_and_snapshot();
    }
}
