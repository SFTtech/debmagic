use std::path::Path;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::{SourceContext, follow_symlinks};
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Severity, SourceRule};
use crate::lint::rules::lintian::fields_version::is_native;
use crate::lint::upstream_metadata::{Mapping, UpstreamMetadata};

pub struct DebianWatchFileIsMissing;

declare_rule! {
    /// The package has no `debian/watch` and its upstream metadata does not
    /// provide an equivalent.
    ///
    /// Non-native packages only. Fires when `debian/watch` is not a file in the
    /// packaging File tree (a symlink to a file counts; a directory does not). It
    /// is not reported when `debian/upstream/metadata` describes a source that
    /// uscan can track without a watch file: `Archive: GitHub` with a GitHub
    /// `Repository` or `Repository-Browse`, `Archive: CPAN` with `Upstream-Name`
    /// or `Name`, or `Archive: Bioconductor` or `CRAN` with the first
    /// `debian/copyright` paragraph's `Upstream-Name`. Severity info; no extra
    /// and no Location. Difference: the copyright is read from its first DEP-5
    /// paragraph only, while lintian also reads the first Deb822 section of a
    /// file that is not DEP-5.
    DebianWatchFileIsMissing,
    code = "LN1344",
    tag = "debian-watch-file-is-missing",
    default_selected = false,
    experimental = false,
}

impl SourceRule for DebianWatchFileIsMissing {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        if is_native(ctx) {
            return;
        }
        let watch = Path::new("debian/watch");
        let has_watch = ctx
            .files()
            .and_then(|files| {
                let entry = files.get(watch)?;
                follow_symlinks(files, entry)
            })
            .is_some();
        if has_watch {
            return;
        }
        if metadata_tracks_upstream(ctx) {
            return;
        }
        ctx.diagnostic(Diagnostic::warn("").with_severity(Severity::Info));
    }
}

/// Lintian's `%templates`: the upstream metadata describes a source uscan can
/// track without a watch file.
fn metadata_tracks_upstream(ctx: &mut SourceContext<'_>) -> bool {
    let Some(metadata) = ctx.upstream_metadata() else {
        return false;
    };
    let UpstreamMetadata::Mapping(mapping) = metadata.as_ref() else {
        return false;
    };
    let Some(archive) = mapping.text("Archive") else {
        return false;
    };
    match archive.to_lowercase().as_str() {
        "github" => github_template(mapping),
        "cpan" => perl_true(mapping.text("Upstream-Name")) || perl_true(mapping.text("Name")),
        "bioconductor" | "cran" => {
            let upstream_name = ctx
                .debian_copyrights()
                .into_iter()
                .find(|(path, _)| path == Path::new("debian/copyright"))
                .and_then(|(_, copyright)| {
                    copyright
                        .paragraphs()
                        .and_then(|paragraphs| paragraphs.first())
                        .map(|paragraph| paragraph.value("Upstream-Name").to_owned())
                });
            perl_true(upstream_name.as_deref())
        }
        _ => false,
    }
}

/// `$src = Repository || Repository-Browse`, a `.git` suffix removed, then a
/// GitHub URL with an owner and a project. Unanchored, as lintian's regex is.
fn github_template(mapping: &Mapping) -> bool {
    static GIT_SUFFIX: std::sync::LazyLock<Regex> =
        std::sync::LazyLock::new(|| Regex::new(r"\.git\s*$").expect("regex"));
    static GITHUB: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
        Regex::new(r"https://github.com/([^/]+)/([^/]+)/?").expect("regex")
    });
    let source = if perl_true(mapping.text("Repository")) {
        mapping.text("Repository")
    } else {
        mapping.text("Repository-Browse")
    };
    let Some(source) = source else {
        return false;
    };
    let source = GIT_SUFFIX.replace(source, "");
    GITHUB.is_match(&source)
}

/// Perl truth of a string: defined, and neither empty nor `"0"`.
fn perl_true(value: Option<&str>) -> bool {
    value.is_some_and(|value| !value.is_empty() && value != "0")
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "Source: example\n\
         Maintainer: Example <ex@example.com>\n\
         Standards-Version: 4.7.2\n\
         \n\
         Package: example\n\
         Architecture: all\n\
         Description: example\n extra\n";

    const COPYRIGHT: &str = "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\n\
         Upstream-Name: rlang\n\
         \n\
         Files: *\n\
         Copyright: 2014, somebody1\n\
         License: gplv1\n something\n";

    fn entry(version: &str) -> String {
        format!(
            "example ({version}) unstable; urgency=low\n\n  * Change.\n\n -- A Uthor <a@example.com>  Tue, 30 Dec 2008 17:34:02 +0000\n"
        )
    }

    fn tree(
        version: &str,
        format: Option<&str>,
        extra: &[(&'static str, TestFile)],
    ) -> Vec<(&'static str, TestFile)> {
        let mut files = vec![
            ("debian/control", TestFile::new(CONTROL)),
            ("debian/changelog", TestFile::new(entry(version))),
        ];
        if let Some(format) = format {
            files.push(("debian/source/format", TestFile::new(format)));
        }
        files.extend(extra.iter().cloned());
        files
    }

    fn watch() -> (&'static str, TestFile) {
        (
            "debian/watch",
            TestFile::new("version=4\nhttps://example.org/ example-(\\d\\S*)\\.tar\\.gz\n"),
        )
    }

    fn metadata(text: &'static str) -> (&'static str, TestFile) {
        ("debian/upstream/metadata", TestFile::new(text))
    }

    fn copyright(text: &'static str) -> (&'static str, TestFile) {
        ("debian/copyright", TestFile::new(text))
    }

    fn package(
        format: &str,
        version: &str,
        dsc_files: &[&str],
        extra: Vec<(&'static str, TestFile)>,
    ) -> SourcePackageCase {
        let mut text = format!(
            "Format: {format}\nSource: example\nBinary: example\nArchitecture: all\nVersion: {version}\n\
             Maintainer: Example <ex@example.com>\nFiles:\n"
        );
        for file in dsc_files {
            text.push_str(&format!(" d41d8cd98f00b204e9800998ecf8427e 0 {file}\n"));
        }
        let dsc = Dsc::parse(&text).expect("valid dsc");
        let mut files = vec![
            ("debian/control", TestFile::new(CONTROL)),
            ("debian/changelog", TestFile::new(entry(version))),
        ];
        files.extend(extra);
        SourcePackageCase::from(files).dsc(dsc)
    }

    #[test]
    fn source() {
        let pass = vec![
            tree("1.0-1", Some("3.0 (quilt)"), &[watch()]),
            tree(
                "1.0-1",
                None,
                &[
                    ("debian/watch", TestFile::symlink("watch.real")),
                    ("debian/watch.real", TestFile::new("version=4\n")),
                ],
            ),
            tree(
                "1.0-1",
                None,
                &[metadata(
                    "Archive: GitHub\nRepository: https://github.com/isaacs/node-glob.git\n",
                )],
            ),
            tree(
                "1.0-1",
                None,
                &[metadata(
                    "Archive: GitHub\nRepository-Browse: https://github.com/a/b/\n",
                )],
            ),
            tree("1.0-1", None, &[metadata("Archive: CPAN\nName: Foo-Bar\n")]),
            tree(
                "1.0-1",
                None,
                &[metadata("Archive: CRAN\n"), copyright(COPYRIGHT)],
            ),
            tree("1.0", Some("3.0 (native)"), &[]),
        ];
        let fail = vec![
            tree("1.0-1", Some("3.0 (quilt)"), &[]),
            tree("1.0-1", None, &[("debian/watch", TestFile::directory())]),
            tree(
                "1.0-1",
                None,
                &[metadata(
                    "Archive: GitHub\nRepository: https://gitlab.com/a/b\n",
                )],
            ),
            tree(
                "1.0-1",
                None,
                &[metadata(
                    "Archive: GitHub\nRepository: ~\nRepository-Browse:\n",
                )],
            ),
            tree("1.0-1", None, &[metadata("Archive: CPAN\nName: ~\n")]),
            tree("1.0-1", None, &[metadata("Archive: CRAN\n")]),
            tree("1.0-1", None, &[metadata("Archive: Unknown\nName: x\n")]),
        ];
        SourceTreeTester::new(DebianWatchFileIsMissing, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![package(
            "3.0 (quilt)",
            "1.0-1",
            &["example_1.0.orig.tar.gz"],
            vec![watch()],
        )];
        let fail = vec![package("1.0", "1.0-1", &["example_1.0-1.diff.gz"], vec![])];
        SourcePackageTester::new(DebianWatchFileIsMissing, pass, fail).test_and_snapshot();
    }
}
