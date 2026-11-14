use std::path::PathBuf;
use std::sync::LazyLock;

use regex::bytes::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::SourceRule;

pub struct ObsoleteUrlInPackaging;

declare_rule! {
    /// A packaging file points at a frozen or closed site.
    ///
    /// Scans regular files named `debian/control`, `debian/copyright`,
    /// `debian/watch`, `debian/upstream`, `debian/upstream/metadata`, or
    /// `debian/upstream-metadata.yaml` in the packaging File tree (Source
    /// tree, or Source package Patched files). Full-line `#` comments are
    /// dropped first. Each hostname in the vendored obsolete-sites list (one
    /// hit, the first URL) matches
    /// `\w+://` plus an optional subdomain, then the hostname, then `/` or
    /// `:`, then the URL up to whitespace or `"<>$`. `ftp://ftp.debian.org`
    /// and `ftp://security.debian.org` match on their own and the extra is
    /// only that host, even when a path follows. Extra is the URL. Location
    /// is the path, with no line.
    ObsoleteUrlInPackaging,
    code = "LN0185",
    tag = "obsolete-url-in-packaging",
    default_selected = true,
    experimental = false,
}

static INTERESTING: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(
        r"^debian/(?:control|copyright|watch|upstream|upstream/metadata|upstream-metadata\.yaml)$",
    )
    .expect("valid regex")
});

static SITE_URLS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    crate::lint::data::lines(include_str!("../../data/obsolete-sites/obsolete-sites"))
        .map(|site| {
            let pattern = format!(
                r#"(?i-u)(\w+://(?:[\w.]*\.)?{site}[/:][^\s"<>$]*)"#,
                site = regex::escape(site)
            );
            Regex::new(&pattern).unwrap_or_else(|error| panic!("obsolete site {site}: {error}"))
        })
        .collect()
});

/// Host only: a following path is not part of the extra.
static DEBIAN_FTP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i-u)(ftp://(?:ftp|security)\.debian\.org)").expect("valid regex")
});

impl SourceRule for ObsoleteUrlInPackaging {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let hits: Vec<(PathBuf, String)> = files
            .query()
            .kinds([EntryKind::RegularFile])
            .path(&INTERESTING)
            .into_iter()
            .filter_map(|entry| {
                let bytes = files.read(entry).ok()?;
                let urls = obsolete_urls(&bytes);
                if urls.is_empty() {
                    None
                } else {
                    Some((entry.path().to_path_buf(), urls))
                }
            })
            .flat_map(|(path, urls)| urls.into_iter().map(move |url| (path.clone(), url)))
            .collect();
        for (path, url) in hits {
            ctx.diagnostic(Diagnostic::warn(url).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

fn obsolete_urls(bytes: &[u8]) -> Vec<String> {
    let text = without_hash_comments(bytes);
    let mut urls = Vec::new();
    for site in SITE_URLS.iter() {
        if let Some(url) = first_url(site, &text) {
            urls.push(url);
        }
    }
    if let Some(url) = first_url(&DEBIAN_FTP, &text) {
        urls.push(url);
    }
    urls
}

fn first_url(pattern: &Regex, text: &[u8]) -> Option<String> {
    let matched = pattern.captures(text)?.get(1)?.as_bytes();
    Some(String::from_utf8_lossy(matched).into_owned())
}

/// Drop lines whose first non-whitespace byte is `#`. The newline stays.
fn without_hash_comments(bytes: &[u8]) -> Vec<u8> {
    let mut kept = Vec::with_capacity(bytes.len());
    for line in bytes.split_inclusive(|&byte| byte == b'\n') {
        let body = line.strip_suffix(b"\n").unwrap_or(line);
        if body.trim_ascii_start().first() == Some(&b'#') {
            if line.ends_with(b"\n") {
                kept.push(b'\n');
            }
            continue;
        }
        kept.extend_from_slice(line);
    }
    kept
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    /// A packaging tree. `debian/control` names the source `example` when the
    /// case does not set `Source` itself, so the Diagnostic carries a package.
    fn tree(files: &[(&str, &str)]) -> Vec<(String, TestFile)> {
        let mut out: Vec<(String, TestFile)> = files
            .iter()
            .map(|(path, body)| {
                let body = if *path == "debian/control" && !body.contains("Source:") {
                    format!("Source: example\n{body}")
                } else {
                    (*body).to_string()
                };
                (path.to_string(), TestFile::new(body))
            })
            .collect();
        if !out.iter().any(|(path, _)| path == "debian/control") {
            out.insert(
                0,
                (
                    "debian/control".to_string(),
                    TestFile::new("Source: example\n"),
                ),
            );
        }
        out
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(&[("debian/control", "Source: example\n")]),
            // Not one of the packaging files this check reads.
            tree(&[(
                "debian/changelog",
                "http://code.google.com/p/foo\nftp://ftp.debian.org\n",
            )]),
            tree(&[("debian/foo.copyright", "Source: https://foo.berlios.de/\n")]),
            tree(&[("README", "http://freshmeat.net/p/foo\n")]),
            // A comment is not a link.
            tree(&[(
                "debian/watch",
                "# http://code.google.com/p/wps-bully/\nversion=4\n",
            )]),
            // http is not the ftp.debian.org case, and a bare host has no `/` or `:`.
            tree(&[(
                "debian/copyright",
                "False-Positive: http://ftp.debian.org/\nSource: https://berlios.de\n",
            )]),
            tree(&[("debian/control", "Homepage: https://github.com/foo/bar\n")]),
            tree(&[("debian/copyright", "Source: https://notberlios.de/\n")]),
            tree(&[(
                "debian/upstream/metadata",
                "FAQ: http://foo.sourceforge.net/faq/\n",
            )]),
            vec![
                (
                    "debian/control".to_string(),
                    TestFile::new("Source: example\n"),
                ),
                (
                    "debian/copyright".to_string(),
                    TestFile::symlink("elsewhere"),
                ),
            ],
            vec![
                (
                    "debian/control".to_string(),
                    TestFile::new("Source: example\n"),
                ),
                (
                    "debian/copyright".to_string(),
                    TestFile::new("Source: https://foo.berlios.de/\n").hardlink(),
                ),
            ],
        ];
        let fail = vec![
            tree(&[("debian/control", "Homepage: http://code.google.com/p/foo\n")]),
            tree(&[(
                "debian/copyright",
                "Source: https://foo.berlios.de/\n\
                 Positive: ftp://ftp.debian.org/debian/doc/\n\
                 Comment: http://freshmeat.net/p/foo\n",
            )]),
            tree(&[(
                "debian/watch",
                "http://gitorious.org/foo/foo/file-(.*)\\.tar\\.gz\n\
                 \n\
                 # http://code.google.com/p/wps-bully/\n",
            )]),
            tree(&[(
                "debian/upstream/metadata",
                "Homepage: http://freecode.com/p/foo\n\
                 Repository: git://gitorious.org/foo/foo.git\n\
                 FAQ: http://foo.sourceforge.net/faq/\n",
            )]),
            tree(&[(
                "debian/upstream",
                "Repository: http://gitorious.org/foo/foo.git\n",
            )]),
            tree(&[(
                "debian/upstream-metadata.yaml",
                "Homepage: http://alioth.debian.org/projects/foo/\n",
            )]),
            tree(&[(
                "debian/copyright",
                "Source: ftp://security.debian.org/debian-security\n",
            )]),
            tree(&[("debian/control", "Homepage: HTTP://Berlios.DE/x\n")]),
            // One hit per hostname: the first URL.
            tree(&[(
                "debian/copyright",
                "Source: https://foo.berlios.de/first\nAlso: https://bar.berlios.de/second\n",
            )]),
            tree(&[(
                "debian/control",
                "Homepage: http://search.cpan.org/Foo-Bar-9.0_01/\n",
            )]),
            // `:` after the host counts, and `$` ends the URL.
            tree(&[(
                "debian/copyright",
                "Source: https://berlios.de:80/x\nSee: http://freshmeat.net/p/foo$bar more\n",
            )]),
            tree(&[(
                "debian/control",
                "Homepage: \"http://code.google.com/p/foo\"\n",
            )]),
            tree(&[
                ("debian/control", "Homepage: http://code.google.com/p/foo\n"),
                ("debian/watch", "http://gitorious.org/foo\n"),
            ]),
        ];
        SourceTreeTester::new(ObsoleteUrlInPackaging, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(vec![(
            "debian/control",
            TestFile::new("Source: example\n"),
        )])];
        let fail = vec![SourcePackageCase::from(vec![(
            "debian/copyright",
            TestFile::new("Source: https://foo.berlios.de/\n"),
        )])];
        SourcePackageTester::new(ObsoleteUrlInPackaging, pass, fail).test_and_snapshot();
    }
}
