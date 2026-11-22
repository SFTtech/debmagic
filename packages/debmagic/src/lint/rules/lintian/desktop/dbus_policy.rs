//! Shared reading of D-Bus policy files for the `desktop/dbus` Rules.
//!
//! Mirrors lintian's `Lintian::Check::Desktop::Dbus`: the policy files are the
//! direct children of the four `*.d` folders, and each file is scanned with the
//! same small state machine over `<policy>`, `</policy>`, `<allow>` and `<deny>`.

use std::path::PathBuf;
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

/// A rule reported by a Rule: the policy file, the 1-based rule number, and the
/// normalized rule text (the extra).
pub(super) struct Hit {
    pub path: PathBuf,
    pub position: usize,
    pub rule: String,
}

/// Every rule of every policy file that `matches` accepts.
pub(super) fn policy_hits(files: &FileTree, matches: impl Fn(&str) -> bool) -> Vec<Hit> {
    let mut hits = Vec::new();
    for entry in policy_files(files) {
        let Some(xml) = policy_text(files, entry) else {
            continue;
        };
        for (position, rule) in policy_rules(&xml) {
            if matches(&rule) {
                hits.push(Hit {
                    path: entry.path().to_path_buf(),
                    position,
                    rule,
                });
            }
        }
    }
    hits
}

/// Lintian's "about sending but no send_destination" test, minus the root
/// "agent" pattern (`user="root"` with an `<allow>`), which is skipped.
pub(super) fn is_unscoped_send(rule: &str) -> bool {
    static ROOT_AGENT: OnceLock<Regex> = OnceLock::new();
    let root_agent = ROOT_AGENT.get_or_init(|| {
        Regex::new(r#"[^>]*user=['"]root['"].*<allow"#).expect("valid root agent regex")
    });
    rule.contains("send_") && !rule.contains("send_destination=") && !root_agent.is_match(rule)
}

/// An unscoped send rule that is not limited to an interface other than
/// `org.freedesktop.DBus.Properties`, and that allows (rather than denies).
pub(super) fn is_excessively_broad(rule: &str) -> bool {
    static PROPERTIES: OnceLock<Regex> = OnceLock::new();
    let properties = PROPERTIES.get_or_init(|| {
        Regex::new(r#"send_interface=['"]org\.freedesktop\.DBus\.Properties['"]"#)
            .expect("valid properties regex")
    });
    let other_interface = rule.contains("send_interface=") && !properties.is_match(rule);
    is_unscoped_send(rule) && !other_interface && rule.contains("<allow")
}
