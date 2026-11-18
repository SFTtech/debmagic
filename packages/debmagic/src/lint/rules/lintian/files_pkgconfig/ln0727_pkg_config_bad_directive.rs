use std::path::PathBuf;
use std::sync::LazyLock;

use regex::{Regex, RegexBuilder};

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct PkgConfigBadDirective;

declare_rule! {
    /// A pkg-config file in `usr/lib/<dir>/pkgconfig/` or `usr/share/pkgconfig/`
    /// has a compiler flag or an unexpanded `@...@` placeholder that does not
    /// belong there.
    ///
    /// Only regular files count, not symlinks or hardlinks. Each `.pc` file is
    /// scanned as one block: `#` comments are removed, backslash-newline joins
    /// lines, then each pattern of lintian's `files/pkg-config-bad-regex` table
    /// reports its first capture once per line. The extra is that capture.
    PkgConfigBadDirective,
    code = "LN0727",
    tag = "pkg-config-bad-directive",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PkgConfigBadDirective {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<(PathBuf, String)> = files
            .query()
            .kinds([EntryKind::RegularFile])
            .path(&PKGCONFIG)
            .into_iter()
            .filter_map(|entry| {
                let bytes = files.read(entry).ok()?;
                Some((
                    entry.path().to_path_buf(),
                    String::from_utf8_lossy(&bytes).into_owned(),
                ))
            })
            .flat_map(|(path, text)| {
                bad_directives(&text)
                    .into_iter()
                    .map(move |context| (path.clone(), context))
            })
            .collect();
        for (path, context) in hits {
            ctx.diagnostic(Diagnostic::error(context).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// Perl's `\V`: any character but vertical whitespace.
const NOT_VERTICAL: &str = r"[^\n\x0B\x0C\r\x{85}\x{2028}\x{2029}]";

/// `usr/(lib(/<dir>)?|share)/pkgconfig/<name>.pc`, with `<name>` one component.
static PKGCONFIG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^usr/(?:lib(?:/[^/]+)?|share)/pkgconfig/[^/]+\.pc$").expect("valid regex")
});

static COMMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!("#{NOT_VERTICAL}*")).expect("valid regex"));

/// Lintian's `files/pkg-config-bad-regex`, compiled as Perl's `/xms`.
static PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    crate::lint::data::lines(include_str!("../../../data/files/pkg-config-bad-regex"))
        .map(|pattern| {
            // `\x20`, not a space: the builder ignores whitespace (`/x`).
            let pattern = pattern
                .replace(r"\h", r"[\t\x20]")
                .replace(r"\V", NOT_VERTICAL);
            RegexBuilder::new(&pattern)
                .ignore_whitespace(true)
                .dot_matches_new_line(true)
                .multi_line(true)
                .build()
                .unwrap_or_else(|error| panic!("pkg-config pattern {pattern:?}: {error}"))
        })
        .collect()
});

/// Lintian's `Files::Pkgconfig` scan over one file, read as one block.
fn bad_directives(text: &str) -> Vec<String> {
    let text = COMMENT.replace_all(text, "").replace("\\\n", " ");
    PATTERNS
        .iter()
        .flat_map(|pattern| {
            pattern
                .captures_iter(&text)
                .map(|caps| caps[1].to_string())
                .collect::<Vec<_>>()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn pc(text: &str) -> TestFile {
        TestFile::new(text)
    }

    fn pkgconfig(text: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![("usr/lib/pkgconfig/libfoo.pc", pc(text))])
    }

    #[test]
    fn one_capture_per_pattern_and_line() {
        // `^`-anchored, greedy: the last `-O` flag on the line wins.
        assert_eq!(bad_directives("Cflags: -O2 -O3\n"), vec!["-O3".to_string()]);
    }

    #[test]
    fn comments_and_continuations() {
        assert!(bad_directives("Cflags: -I${includedir} # -pie\n").is_empty());
        assert_eq!(
            bad_directives("Cflags: -O2 \\\n    -pie\n"),
            vec!["-O2".to_string(), "-pie".to_string()]
        );
    }

    #[test]
    fn pkgconfig_paths() {
        let check = |name: &str| PKGCONFIG.is_match(name);
        assert!(check("usr/lib/pkgconfig/libfoo.pc"));
        assert!(check("usr/lib/x86_64-linux-gnu/pkgconfig/libfoo.pc"));
        assert!(check("usr/share/pkgconfig/libfoo.pc"));
        assert!(!check("usr/lib/x86_64-linux-gnu/pkgconfig/libfoo.pc.in"));
        assert!(!check("usr/lib/x86_64-linux-gnu/pkgconfig/sub/libfoo.pc"));
        assert!(!check("usr/lib/libfoo.pc"));
        assert!(!check("usr/lib/pkgconfig/.pc"));
    }

    #[test]
    fn test() {
        let pass = vec![
            pkgconfig("Name: foo\nCflags: -DFOO=1 -I${includedir}\nLibs: -L${libdir} -lfoo\n"),
            pkgconfig("Description: uses -pie\n"),
            pkgconfig("Cflags: -I${includedir} # -pie\n"),
            BinaryPackageCase::from(vec![("usr/lib/libfoo.pc", pc("Cflags: -pie \n"))]),
            BinaryPackageCase::from(vec![(
                "usr/lib/pkgconfig/link.pc",
                TestFile::symlink("libfoo.pc"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/pkgconfig/hard.pc",
                pc("Cflags: -pie \n").hardlink(),
            )]),
        ];
        let fail = vec![
            pkgconfig("Cflags: -pie \n"),
            pkgconfig("Name: @FOO@\n"),
            pkgconfig("Cflags: -I/usr/local/x\n"),
            pkgconfig("Cflags: -O2 \\\n    -pie\n"),
            BinaryPackageCase::from(vec![("usr/share/pkgconfig/libfoo.pc", pc("Cflags: -g\n"))]),
            BinaryPackageCase::from(vec![(
                "usr/lib/x86_64-linux-gnu/pkgconfig/libfoo.pc",
                pc("Cflags: -pie \n"),
            )])
            .udeb(),
        ];
        BinaryPackageTester::new(PkgConfigBadDirective, pass, fail).test_and_snapshot();
    }
}
