//! How lintian's `files/symbolic-links` Check judges the link text of a
//! symlink in a Binary package (`visit_installed_files`).

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::context::BinaryPackageContext;
use crate::lint::file_tree::EntryKind;

static DOUBLE_SLASH: LazyLock<Regex> = LazyLock::new(|| Regex::new("//+").expect("valid regex"));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SymlinkIssue {
    SpuriousSegments,
    SelfRecursive,
    Lengthy,
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
pub(super) fn symlink_issues(name: &str, link: &str) -> Vec<SymlinkIssue> {
    let mut issues = Vec::new();
    let mut target = DOUBLE_SLASH.replace_all(link, "/").into_owned();
    if target.len() > 1 && target.ends_with('/') {
        target.pop();
    }
    if target.starts_with('/') {
        if target == "/" {
            issues.push(SymlinkIssue::SelfRecursive);
        }
        return issues;
    }

    let mut file_components = split_path(name);
    file_components.pop();
    let mut link_components: VecDeque<&str> = split_path(&target).into();
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
            // too many up segments: lintian stops judging this link
            None => return issues,
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
    if link_components
        .iter()
        .any(|component| *component == ".." || *component == ".")
    {
        issues.push(SymlinkIssue::SpuriousSegments);
    }
    issues
}

/// Symlinks of the Data files whose link text has `issue`, with their link
/// text, once per hint.
pub(super) fn symlinks_with_issue(
    ctx: &mut BinaryPackageContext<'_>,
    issue: SymlinkIssue,
) -> Vec<(PathBuf, String)> {
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
        for found_issue in symlink_issues(name, &link) {
            if found_issue == issue {
                found.push((entry.path().to_path_buf(), link.to_string()));
            }
        }
    }
    found
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
                &[SelfRecursive],
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
            assert_eq!(symlink_issues(name, link), *expected, "{name} -> {link}");
        }
    }

    #[test]
    fn plain_links_have_no_issue() {
        assert!(symlink_issues("usr/bin/tool", "../lib/tool/tool").is_empty());
        assert!(symlink_issues("usr/bin/tool", "/usr/lib/tool").is_empty());
        assert!(symlink_issues("usr/bin/tool", "tool-1.0").is_empty());
    }

    #[test]
    fn too_many_up_segments_stop_the_judgement() {
        assert!(symlink_issues("a/link", "../../x/./y").is_empty());
        assert!(symlink_issues("link", "../x/./y").is_empty());
    }

    #[test]
    fn slashes_are_normalised_first() {
        assert_eq!(symlink_issues("usr/bin/x", "//"), [SelfRecursive]);
        assert_eq!(symlink_issues("usr/bin/x", "./a/"), [SpuriousSegments]);
    }
}
