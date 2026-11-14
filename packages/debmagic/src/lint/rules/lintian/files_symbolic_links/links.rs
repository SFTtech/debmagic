//! How lintian's `files/symbolic-links` Check judges the link text of a
//! symlink in a Binary package (`visit_installed_files`).

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::Severity;

static DOUBLE_SLASH: LazyLock<Regex> = LazyLock::new(|| Regex::new("//+").expect("valid regex"));

/// `files/compressed-file-extensions`, in file order, as the Check joins them.
static COMPRESSED_EXT: LazyLock<Regex> = LazyLock::new(|| {
    let extensions = crate::lint::data::lines(include_str!(
        "../../../data/files/compressed-file-extensions"
    ))
    .map(regex::escape)
    .collect::<Vec<_>>()
    .join("|");
    Regex::new(&format!(r"[.]({extensions})\s*$")).expect("valid regex")
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SymlinkIssue {
    SpuriousSegments,
    SelfRecursive,
    Lengthy,
    DoubleSlash,
    EndsWithSlash,
    AbsoluteInTopLevel,
    TooManyUpSegments,
    Relative,
    CompressedWrongExt,
}

/// Perl's `split m{/}`: trailing empty fields are dropped.
fn split_path(value: &str) -> Vec<&str> {
    let mut parts: Vec<&str> = value.split('/').collect();
    while parts.last() == Some(&"") {
        parts.pop();
    }
    parts
}

/// Every issue found for one symlink, once per hint lintian emits, in order.
/// `name` is the symlink's path in the package, `link` its target as written.
/// `udeb` suppresses `absolute-symlink-in-top-level-folder` and the udeb half
/// of `relative-symlink`.
pub(super) fn symlink_issues(name: &str, link: &str, udeb: bool) -> Vec<SymlinkIssue> {
    let mut issues = Vec::new();
    if DOUBLE_SLASH.is_match(link) {
        issues.push(SymlinkIssue::DoubleSlash);
    }
    let mut target = DOUBLE_SLASH.replace_all(link, "/").into_owned();
    if target.len() > 1 && target.ends_with('/') {
        issues.push(SymlinkIssue::EndsWithSlash);
        target.pop();
    }
    let filetop = name.trim_start_matches('/').split('/').next().unwrap_or("");

    if let Some(rest) = target.strip_prefix('/') {
        let linktop = rest.split('/').next().unwrap_or("");
        if !udeb && filetop == linktop {
            issues.push(SymlinkIssue::AbsoluteInTopLevel);
        }
        if target == "/" {
            issues.push(SymlinkIssue::SelfRecursive);
        }
    } else {
        judge_relative(&mut issues, name, &target, filetop, udeb);
    }
    if compressed_wrong(name, link) {
        issues.push(SymlinkIssue::CompressedWrongExt);
    }
    issues
}

/// Relative-target half of `visit_installed_files`. Returns after too many
/// `..` segments, matching lintian's `goto NEXT_LINK`.
fn judge_relative(
    issues: &mut Vec<SymlinkIssue>,
    name: &str,
    target: &str,
    filetop: &str,
    udeb: bool,
) {
    let mut file_components = split_path(name);
    file_components.pop();
    let mut link_components: VecDeque<&str> = split_path(target).into();
    let mut last_pop: Option<&str> = None;
    let mut current: Option<&str>;
    loop {
        current = link_components.pop_front();
        // Perl's `while ($c = shift ...)` also stops on "" and "0".
        let Some(component) =
            current.filter(|component| !component.is_empty() && *component != "0")
        else {
            break;
        };
        if component == "." {
            if target != "." {
                issues.push(SymlinkIssue::SpuriousSegments);
            }
            continue;
        }
        if component != ".." {
            break;
        }
        match file_components.pop() {
            Some(popped) => last_pop = Some(popped),
            None => {
                issues.push(SymlinkIssue::TooManyUpSegments);
                return;
            }
        }
    }
    if current.is_none() {
        issues.push(SymlinkIssue::SelfRecursive);
    }
    if let (Some(popped), Some(component)) = (last_pop, current)
        && popped == component
    {
        issues.push(SymlinkIssue::Lengthy);
    }
    if file_components.is_empty() {
        // `&&` binds tighter than `||` in the Check, and `undef` stringifies
        // to "" for `ne`.
        let crosses = match current {
            None => !filetop.is_empty(),
            Some(component) => filetop != component,
        };
        if (!udeb && current.is_none()) || crosses {
            issues.push(SymlinkIssue::Relative);
        }
    }
    if link_components
        .iter()
        .any(|component| *component == ".." || *component == ".")
    {
        issues.push(SymlinkIssue::SpuriousSegments);
    }
}

fn compressed_wrong(name: &str, link: &str) -> bool {
    let Some(extension) = COMPRESSED_EXT
        .captures(link)
        .and_then(|caps| caps.get(1))
        .map(|matched| matched.as_str())
    else {
        return false;
    };
    let suffix = format!(".{extension}");
    let trimmed = name.trim_end_matches(|character: char| character.is_whitespace());
    !trimmed.ends_with(&suffix)
}

/// Symlinks of the Data files whose link text has `issue`, with their link
/// text, once per hint.
pub(super) fn symlinks_with_issue(
    ctx: &mut BinaryPackageContext<'_>,
    issue: SymlinkIssue,
) -> Vec<(PathBuf, String)> {
    let udeb = ctx.is_udeb();
    let Some(files) = ctx.data_files() else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for entry in files.entries() {
        if entry.kind() != EntryKind::Symlink {
            continue;
        }
        let (Some(name), Some(target)) = (entry.path().to_str(), entry.link_target()) else {
            continue;
        };
        let link = target.to_string_lossy();
        for found_issue in symlink_issues(name, &link, udeb) {
            if found_issue == issue {
                found.push((entry.path().to_path_buf(), link.to_string()));
            }
        }
    }
    found
}

pub(super) fn emit_issue(
    ctx: &mut BinaryPackageContext<'_>,
    issue: SymlinkIssue,
    severity: Severity,
) {
    for (path, link) in symlinks_with_issue(ctx, issue) {
        ctx.diagnostic(
            Diagnostic::error(link)
                .with_severity(severity)
                .with_location(Location {
                    path,
                    line: None,
                    column: None,
                }),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::SymlinkIssue::*;
    use super::*;

    #[test]
    fn matches_lintian_on_its_recipes() {
        let cases: &[(&str, &str, &[SymlinkIssue])] = &[
            (
                "usr/share/lintian/data/self-recursive",
                "/",
                &[SelfRecursive],
            ),
            (
                "usr/share/lintian/data/spurious",
                "../../lintian-old/../lintian/data/data-file",
                &[SpuriousSegments],
            ),
            (
                "usr/share/doc/binary/html/ch2.html",
                "../html/./ch1.html",
                &[Lengthy, SpuriousSegments],
            ),
            ("usr/share/doc/bar/star", "../bar/foo", &[Lengthy]),
            (
                "usr/lib/filenames/symlink10wrong",
                "../../..",
                &[SelfRecursive, Relative],
            ),
            (
                "usr/share/lintian/data/pkg.conf",
                "../..//..//..//../etc/lintian/pkg.conf",
                &[DoubleSlash, TooManyUpSegments],
            ),
            ("usr/share/etc-lintian", "/etc/lintian/", &[EndsWithSlash]),
            (
                "usr/share/lintian/data/pkg-old.conf",
                "../../../../etc/lintian/pkg.conf",
                &[Relative],
            ),
            (
                "usr/lib/lintian/data/comp-data",
                "../../../share/lintian/data/comp-data.gz",
                &[CompressedWrongExt],
            ),
            (
                "usr/share/lintian/data/data-file.old",
                "/usr/share/lintian/data/data-file",
                &[AbsoluteInTopLevel],
            ),
            (
                "usr/lib/filenames/symlink3wrong",
                "/usr/lib/filenames/symlink2",
                &[AbsoluteInTopLevel],
            ),
            (
                "usr/share/doc/libbaz2/changelog.gz",
                "/usr/share/doc/lintian/changelog.gz",
                &[AbsoluteInTopLevel],
            ),
            ("usr/lib/filenames/symlink5ok+warn", "..", &[SelfRecursive]),
            ("usr/lib/filenames/symlink6ok+warn", ".", &[SelfRecursive]),
            (
                "usr/lib/filenames/symlink6wrong",
                "./file4",
                &[SpuriousSegments],
            ),
            (
                "usr/lib/filenames/symlink7wrong",
                "../menu/./something",
                &[SpuriousSegments],
            ),
            (
                "usr/lib/filenames/symlink5wrong",
                "../menu/../somethingelse",
                &[SpuriousSegments],
            ),
            (
                "usr/lib/filenames/symlink4wrong",
                "../filenames/symlink2",
                &[Lengthy],
            ),
        ];
        for (name, link, expected) in cases {
            assert_eq!(
                symlink_issues(name, link, false),
                *expected,
                "{name} -> {link}"
            );
        }
    }

    #[test]
    fn plain_links_have_no_issue() {
        assert!(symlink_issues("usr/bin/tool", "../lib/tool/tool", false).is_empty());
        assert!(symlink_issues("usr/bin/tool", "/var/lib/tool", false).is_empty());
        assert!(symlink_issues("usr/bin/tool", "tool-1.0", false).is_empty());
    }

    #[test]
    fn too_many_up_segments_stop_the_judgement() {
        assert_eq!(
            symlink_issues("a/link", "../../x/./y", false),
            [TooManyUpSegments]
        );
        assert_eq!(
            symlink_issues("link", "../x/./y", false),
            [TooManyUpSegments]
        );
    }

    #[test]
    fn slashes_are_normalised_first() {
        assert_eq!(
            symlink_issues("usr/bin/x", "//", false),
            [DoubleSlash, SelfRecursive]
        );
        assert_eq!(
            symlink_issues("usr/bin/x", "./a/", false),
            [EndsWithSlash, SpuriousSegments]
        );
    }

    #[test]
    fn udeb_skips_absolute_in_the_same_top_level() {
        assert!(
            symlink_issues(
                "usr/share/lintian/data/data-file.old",
                "/usr/share/lintian/data/data-file",
                true
            )
            .is_empty()
        );
    }
}
