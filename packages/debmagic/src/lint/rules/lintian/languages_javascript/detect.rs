//! Whether pkg-js-tools will run a build-time test, and which file it finds.

use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::context::SourceContext;
use crate::lint::file_tree::{EntryKind, FileTree};

use super::super::rules_and_control::{
    build_depends_all_names, names_package, read_rules, rules_lines,
};

static DH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:\s+[@+-]?(?:\S+=\S+\s+)*)dh\s+").expect("regex"));
static WITH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s--with(?:=|\s+)(\S+)").expect("regex"));
static OVERRIDE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([^:]*override_dh_[^:]*):").expect("regex"));
static DYNAMIC: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\$[({]\w").expect("regex"));
static NONEMPTY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^[^#]*\w").expect("regex"));

const CANDIDATES: [&str; 2] = ["debian/nodejs/test", "debian/tests/pkg-js/test"];

pub(super) enum NodejsTest {
    Missing,
    Present { path: PathBuf, empty: bool },
}

/// `None` when nodejs is not driving the build test, or `debian/rules` is absent.
pub(super) fn nodejs_test(ctx: &mut SourceContext<'_>) -> Option<NodejsTest> {
    let rules = {
        let files = ctx.files()?;
        read_rules(files)?
    };
    if !uses_nodejs(ctx, &rules) || suppresses(&rules) {
        return None;
    }
    let files = ctx.files()?;
    for candidate in CANDIDATES {
        if let Some(empty) = file_is_empty(files, candidate) {
            return Some(NodejsTest::Present {
                path: PathBuf::from(candidate),
                empty,
            });
        }
    }
    Some(NodejsTest::Missing)
}

fn uses_nodejs(ctx: &SourceContext<'_>, rules: &str) -> bool {
    let names = ctx
        .debian_control()
        .map(|control| build_depends_all_names(control.source()))
        .unwrap_or_default();
    if names_package(&names, "dh-sequence-nodejs") {
        return true;
    }
    rules_lines(rules)
        .into_iter()
        .any(|(_, line)| with_nodejs(&line))
}

fn with_nodejs(line: &str) -> bool {
    if !DH.is_match(line) {
        return false;
    }
    WITH.captures_iter(line).any(|captures| {
        unquote(&captures[1])
            .split(',')
            .any(|addon| addon == "nodejs")
    })
}

/// `dh` with `$(…)` / `${…}`, or the Check's `auto_test` capture.
///
/// The capture includes the `override_dh_` prefix, so `override_dh_auto_test:`
/// does not suppress. That matches the Check.
fn unquote(token: &str) -> &str {
    let bytes = token.as_bytes();
    if bytes.len() >= 2 {
        let quote = bytes[0];
        if (quote == b'\'' || quote == b'"') && bytes[bytes.len() - 1] == quote {
            return &token[1..token.len() - 1];
        }
    }
    token
}

fn suppresses(rules: &str) -> bool {
    rules_lines(rules).into_iter().any(|(_, line)| {
        (DH.is_match(&line) && DYNAMIC.is_match(&line))
            || OVERRIDE
                .captures(&line)
                .is_some_and(|captures| &captures[1] == "auto_test")
    })
}

/// `Some` when the path is a readable file. `true` when no line has a word
/// character before a `#`.
fn file_is_empty(files: &FileTree, path: &str) -> Option<bool> {
    let entry = files.get(path)?;
    if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
        return None;
    }
    let bytes = files.read(entry).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    Some(!NONEMPTY.is_match(&text))
}
