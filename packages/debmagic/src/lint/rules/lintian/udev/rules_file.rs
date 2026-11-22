//! Shared reading of udev rule files for the `udev` Check (lintian's `installable`
//! and `check_udev_rules`). Used by the Rules of that Check, not a Rule itself.

use std::collections::VecDeque;
use std::path::PathBuf;

use regex::Regex;

use crate::lint::context::{BinaryPackageContext, follow_symlinks};
use crate::lint::file_tree::{EntryKind, FileTree};

/// Limit on symlinks followed while resolving the rules directory (lintian has none; loops end here).
const MAX_LINKS: u8 = 8;

/// A direct child of `usr/lib/udev/rules.d/` or `lib/udev/rules.d/`.
pub(super) struct RulesFile {
    pub(super) path: PathBuf,
    /// lintian's `is_open_ok`: a regular file or hardlink, possibly behind an in-tree symlink.
    pub(super) readable: bool,
    /// The decoded text; empty when unreadable or not UTF-8, as lintian's `decoded_utf8`.
    pub(super) text: String,
}

/// One logical udev rule: continuation lines joined, comments skipped.
pub(super) struct LogicalRule {
    /// 1-based physical line where the rule ends (lintian's `$position`).
    pub(super) position: u32,
    /// Set after a `GOTO` on a `SUBSYSTEM!=` line, until the next `LABEL` (lintian's `$in_goto`).
    pub(super) in_goto: bool,
    pub(super) text: String,
}

/// Every child of the two udev rules directories, in path order.
pub(super) fn rules_files(ctx: &mut BinaryPackageContext<'_>) -> Vec<RulesFile> {
    let Some(tree) = ctx.data_files() else {
        return Vec::new();
    };
    let mut files = Vec::new();
    for lib_dir in ["usr/lib", "lib"] {
        let Some(dir) = resolve_dir(tree, &format!("{lib_dir}/udev/rules.d")) else {
            continue;
        };
        for child in tree.entries() {
            if child.path().parent() != Some(dir.as_path()) {
                continue;
            }
            let (readable, text) = match follow_symlinks(tree, child) {
                Some(target) => {
                    let text = tree
                        .read(target)
                        .ok()
                        .and_then(|bytes| String::from_utf8(bytes).ok())
                        .unwrap_or_default();
                    (true, text)
                }
                None => (false, String::new()),
            };
            files.push(RulesFile {
                path: child.path().to_path_buf(),
                readable,
                text,
            });
        }
    }
    files
}

/// lintian's `check_udev_rules` loop: joins `\` continuations, skips `#` comments,
/// tracks `$in_goto`. A continuation left open at the end of the file is dropped.
pub(super) fn logical_rules(text: &str) -> Vec<LogicalRule> {
    let continuation = Regex::new(r"^(.*)\\$").expect("valid regex");
    let label = Regex::new(r#"LABEL="[^"]+""#).expect("valid regex");
    let subsystem_not = Regex::new(r#"SUBSYSTEM!="[^"]+""#).expect("valid regex");
    let goto = Regex::new(r#"GOTO="[^"]+""#).expect("valid regex");

    let mut rules = Vec::new();
    let mut continued = String::new();
    let mut in_goto = false;
    // Perl's split(/\n/) drops trailing empty fields; the line numbers depend on that.
    let mut lines: Vec<&str> = text.split('\n').collect();
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    for (index, physical) in lines.into_iter().enumerate() {
        let position = (index + 1) as u32;
        let mut line = physical.to_string();
        if !continued.is_empty() {
            line = format!("{continued}{line}");
            continued.clear();
        }
        if let Some(captures) = continuation.captures(&line) {
            continued = captures[1].to_string();
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        if label.is_match(&line) {
            in_goto = false;
        }
        if subsystem_not.is_match(&line) && goto.is_match(&line) {
            in_goto = true;
        }
        rules.push(LogicalRule {
            position,
            in_goto,
            text: line,
        });
    }
    rules
}

/// Resolves `dir` (relative, `/`-separated) the way lintian's `resolve_path` does: each
/// component that is a symlink in the tree is replaced by its target. Missing directory
/// entries are fine, since children are found by parent path. `None` for absolute,
/// escaping, or looping links.
fn resolve_dir(tree: &FileTree, dir: &str) -> Option<PathBuf> {
    let mut pending: VecDeque<String> = dir.split('/').map(str::to_string).collect();
    let mut resolved = PathBuf::new();
    let mut links = 0u8;
    while let Some(part) = pending.pop_front() {
        match part.as_str() {
            "" | "." => {}
            ".." => {
                if !resolved.pop() {
                    return None;
                }
            }
            name => {
                let candidate = resolved.join(name);
                match tree.get(&candidate) {
                    Some(entry) if entry.kind() == EntryKind::Symlink => {
                        links += 1;
                        if links > MAX_LINKS {
                            return None;
                        }
                        let target = entry.link_target()?;
                        if target.is_absolute() {
                            return None;
                        }
                        // The target is relative to the link's parent, which is `resolved`.
                        for part in target.to_str()?.split('/').rev() {
                            pending.push_front(part.to_string());
                        }
                    }
                    _ => resolved = candidate,
                }
            }
        }
    }
    Some(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_continuations_and_skips_comments() {
        let text = "# c\nSUBSYSTEM==\"usb\", \\\n    MODE=\"0666\"\n\nLABEL=\"x\"\n";
        let rules = logical_rules(text);
        let got: Vec<(u32, &str)> = rules
            .iter()
            .map(|rule| (rule.position, rule.text.as_str()))
            .collect();
        assert_eq!(
            got,
            vec![
                (3, "SUBSYSTEM==\"usb\",     MODE=\"0666\""),
                (4, ""),
                (5, "LABEL=\"x\""),
            ]
        );
    }

    #[test]
    fn goto_sets_in_goto_until_label() {
        let text = "SUBSYSTEM!=\"usb\", GOTO=\"t\"\nATTR{a}==\"1\"\nLABEL=\"t\"\nATTR{a}==\"1\"\n";
        let flags: Vec<bool> = logical_rules(text).iter().map(|r| r.in_goto).collect();
        assert_eq!(flags, vec![true, true, false, false]);
    }

    #[test]
    fn open_continuation_is_dropped() {
        assert!(logical_rules("A=\"1\" \\").is_empty());
    }
}
