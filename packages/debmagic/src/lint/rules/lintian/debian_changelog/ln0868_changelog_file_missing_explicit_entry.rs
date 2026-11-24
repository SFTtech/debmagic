use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::source::{changelog_location, package_is_native};
use super::version::parse_version;

pub struct ChangelogFileMissingExplicitEntry;

declare_rule! {
    /// A backport or source NMU version has no changelog entry of its own.
    ///
    /// Compares the newest and the previous version (both must parse for the
    /// package's nativeness). The expected previous version is the previous
    /// literal, unless the newest version has a backport suffix
    /// (`+debNuM` or `~debNuM`, where neither N nor M is `0`, and the revision
    /// without the suffix is not `0`) or is the first source NMU of a revised
    /// package (`.1` on a revision with a digit, not `0`).
    /// Then it is the newest version without that suffix. The Rule fires when the
    /// previous version differs from the expected one. Extra is
    /// `previous -> expected (missing) -> newest`; the newest heading is the
    /// Location. Changelogs with fewer than two entries are not checked.
    ChangelogFileMissingExplicitEntry,
    code = "LN0868",
    tag = "changelog-file-missing-explicit-entry",
    default_selected = true,
    experimental = false,
}

/// Lintian's `Lintian::Changelog::Version` pieces that this Check needs.
struct Pieces {
    literal: String,
    epoch: String,
    upstream: String,
    maintainer_revision: String,
    source_nmu: String,
}

static NON_NATIVE_PIECES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?:([0-9]+):)?([A-Za-z0-9.+~-]+?)(?:-([A-Za-z0-9.+~]+?)(?:\.([A-Za-z0-9.+~]+))?)?(?:\+b[0-9]+)?$",
    )
    .expect("valid regex")
});

static NATIVE_PIECES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:([0-9]+):)?()([A-Za-z0-9.+~]+?)(?:\+nmu([A-Za-z0-9.+~]+))?(?:\+b[0-9]+)?$")
        .expect("valid regex")
});

static BACKPORT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.*)[+~]deb(\d+)u(\d+)$").expect("valid regex"));

fn pieces(literal: &str, native: bool) -> Option<Pieces> {
    parse_version(literal, native)?;
    let caps = if native {
        NATIVE_PIECES.captures(literal)?
    } else {
        NON_NATIVE_PIECES.captures(literal)?
    };
    let group = |index: usize| {
        caps.get(index)
            .map_or("", |found| found.as_str())
            .to_string()
    };
    // Both patterns put the Debian revision in group 3 and the source NMU in group 4;
    // native packages have an empty upstream in group 2.
    Some(Pieces {
        literal: literal.to_string(),
        epoch: group(1),
        upstream: group(2),
        maintainer_revision: group(3),
        source_nmu: group(4),
    })
}

impl Pieces {
    fn epoch_string(&self) -> String {
        if self.epoch.is_empty() {
            String::new()
        } else {
            format!("{}:", self.epoch)
        }
    }

    fn upstream_string(&self) -> String {
        if self.upstream.is_empty() {
            String::new()
        } else {
            format!("{}-", self.upstream)
        }
    }

    /// Lintian's `without_source_nmu`.
    fn without_source_nmu(&self) -> String {
        format!(
            "{}{}{}",
            self.epoch_string(),
            self.upstream_string(),
            self.maintainer_revision
        )
    }

    /// Lintian's `without_backport`, with the backport suffix and whether it was present.
    fn without_backport(&self) -> (String, bool) {
        let (debian_without_backport, has_backport) =
            match BACKPORT.captures(&self.maintainer_revision) {
                Some(caps) => {
                    let release = caps.get(2).map_or("", |found| found.as_str());
                    let revision = caps.get(3).map_or("", |found| found.as_str());
                    let stem = caps.get(1).map_or("", |found| found.as_str()).to_string();
                    (
                        stem,
                        !release.is_empty()
                            && !revision.is_empty()
                            && release != "0"
                            && revision != "0",
                    )
                }
                None => (self.maintainer_revision.clone(), false),
            };
        let text = format!(
            "{}{}{}",
            self.epoch_string(),
            self.upstream_string(),
            debian_without_backport
        );
        (text, has_backport && debian_without_backport != "0")
    }
}

impl SourceRule for ChangelogFileMissingExplicitEntry {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let native = package_is_native(ctx);
        let Some((latest_literal, previous_literal, line)) =
            ctx.debian_changelog().and_then(|changelog| {
                let [latest, previous, ..] = changelog.entries() else {
                    return None;
                };
                Some((
                    latest.version().to_string(),
                    previous.version().to_string(),
                    latest.line(),
                ))
            })
        else {
            return;
        };
        let (Some(latest), Some(previous)) = (
            pieces(&latest_literal, native),
            pieces(&previous_literal, native),
        ) else {
            return;
        };

        let mut expected = previous.literal.clone();
        let (without_backport, backport) = latest.without_backport();
        if backport {
            expected = without_backport;
        }
        if latest.source_nmu == "1"
            && latest
                .maintainer_revision
                .chars()
                .any(|c| c.is_ascii_digit())
            && latest.maintainer_revision != "0"
        {
            expected = latest.without_source_nmu();
        }

        if previous.literal == expected {
            return;
        }
        ctx.diagnostic(
            Diagnostic::warn(format!(
                "{} -> {} (missing) -> {}",
                previous.literal, expected, latest.literal
            ))
            .with_location(changelog_location(line)),
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn entry(source: &str, version: &str, date: &str) -> String {
        format!(
            "{source} ({version}) unstable; urgency=low\n\n  * Change.\n\n -- A Uthor <a@example.com>  {date}\n"
        )
    }

    fn tree(format: &str, changelog: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/source/format", TestFile::new(format)),
            ("debian/changelog", TestFile::new(changelog)),
        ]
    }

    const NEW: &str = "Tue, 30 Dec 2008 17:34:02 +0000";
    const OLD: &str = "Fri, 06 Feb 2008 22:22:37 -0800";

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            tree("3.0 (quilt)\n", &entry("example", "1.0-1", NEW)),
            tree(
                "3.0 (quilt)\n",
                &format!(
                    "{}{}",
                    entry("example", "1.0-2", NEW),
                    entry("example", "1.0-1", OLD)
                ),
            ),
            // A backport with the previous version present.
            tree(
                "3.0 (quilt)\n",
                &format!(
                    "{}{}",
                    entry("example", "1.2-3+deb9u1", NEW),
                    entry("example", "1.2-3", OLD)
                ),
            ),
            // A first source NMU with the previous version present.
            tree(
                "3.0 (quilt)\n",
                &format!(
                    "{}{}",
                    entry("example", "1.2-3.1", NEW),
                    entry("example", "1.2-3", OLD)
                ),
            ),
        ];
        let fail = vec![
            tree(
                "3.0 (quilt)\n",
                &format!(
                    "{}{}",
                    entry("example", "1.2-3+deb9u1", NEW),
                    entry("example", "0.9-1", OLD)
                ),
            ),
            tree(
                "3.0 (quilt)\n",
                &format!(
                    "{}{}",
                    entry("example", "1.2-3.1", NEW),
                    entry("example", "0.9-1", OLD)
                ),
            ),
        ];
        SourceTreeTester::new(ChangelogFileMissingExplicitEntry, pass, fail).test_and_snapshot();
    }
}
