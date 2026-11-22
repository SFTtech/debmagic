//! Shared reading of D-Bus policy files for the `desktop/dbus` Rules.
//!
//! Mirrors lintian's `Lintian::Check::Desktop::Dbus`: the policy files are the
//! direct children of the four `*.d` folders, and each file is scanned with the
//! same small state machine over `<policy>`, `</policy>`, `<allow>` and `<deny>`.

use std::sync::OnceLock;

use regex::Regex;

use crate::lint::file_tree::{Entry, EntryKind, FileTree};

const POLICY_DIRS: [&str; 4] = [
    "etc/dbus-1/session.d",
    "etc/dbus-1/system.d",
    "usr/share/dbus-1/session.d",
    "usr/share/dbus-1/system.d",
];

/// Regular files directly inside the policy folders, in tree order.
pub(super) fn policy_files(files: &FileTree) -> Vec<&Entry> {
    files
        .entries()
        .iter()
        .filter(|entry| {
            matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink)
                && entry
                    .path()
                    .parent()
                    .is_some_and(|parent| POLICY_DIRS.iter().any(|dir| parent == *dir))
        })
        .collect()
}

/// The file as UTF-8 text. Empty and non-UTF-8 files yield `None`, as lintian's
/// `decoded_utf8` yields an empty string for them.
pub(super) fn policy_text(files: &FileTree, entry: &Entry) -> Option<String> {
    let bytes = files.read(entry).ok()?;
    let text = std::str::from_utf8(&bytes).ok()?;
    (!text.is_empty()).then(|| text.to_string())
}

/// Each `<allow>` or `<deny>` rule, prefixed by its enclosing `<policy ...>`
/// opening tag, with whitespace runs collapsed to one space. Positions are
/// 1-based, counted over all rules of the file, as lintian's pointers are.
pub(super) fn policy_rules(xml: &str) -> Vec<(usize, String)> {
    static TOKEN: OnceLock<Regex> = OnceLock::new();
    let token = TOKEN.get_or_init(|| {
        Regex::new(r"(?s)(<policy[^>]*>)|(</policy\s*>)|(<(?:allow|deny)[^>]*>)")
            .expect("valid policy token regex")
    });
    static WHITESPACE: OnceLock<Regex> = OnceLock::new();
    let whitespace = WHITESPACE.get_or_init(|| Regex::new(r"\s+").expect("valid whitespace regex"));

    let mut policy = String::new();
    let mut rules = Vec::new();
    for caps in token.captures_iter(xml) {
        if let Some(open) = caps.get(1) {
            policy = open.as_str().to_string();
        } else if caps.get(2).is_some() {
            policy.clear();
        } else if let Some(rule) = caps.get(3) {
            rules.push(format!("{}{}", policy, rule.as_str()));
        }
    }
    rules
        .into_iter()
        .enumerate()
        .map(|(index, rule)| (index + 1, whitespace.replace_all(&rule, " ").into_owned()))
        .collect()
}
