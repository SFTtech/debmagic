//! Diversions declared by the maintainer scripts of a Binary package.
//!
//! This is what lintian 2.139.0's `maintainer-scripts/diversion` Check reads
//! from Control files. Every code line of a maintainer script (continuations
//! joined on every interpreter, see [`maintainer_script`]) that calls
//! `dpkg-divert` in command position, and is not `--help`, `--list`,
//! `--truename`, or `--version`, declares one Diversion. It removes when the
//! line says `--remove` and adds otherwise. The diverted path is what is left
//! once lintian has dropped the options it knows (`--package`, `--divert`, and
//! `--admindir` with their argument; `--add`, `--remove`, `--rename`,
//! `--no-rename`, `--quiet`, `--test`, `--local`), shell variables in front of
//! them, unpaired parentheses and braces, and everything from `&&`, `||`, `;`,
//! or a redirection after the last quoted string. Quotes and the leading `/`
//! go, too.
//!
//! The path is kept the way lintian keys it: quoted as a Perl regex, with each
//! shell expansion (`$var`, `${…}`, `$(…)`) as `.+`. One expansion anywhere
//! puts the whole package into wildcard mode: paths are then merged to the
//! widest pattern that matches them and are looked up as unanchored regexes.
//!
//! [`maintainer_script`]: super::maintainer_script

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::{NoExpand, Regex};

use super::file_tree::{EntryKind, FileTree};
use super::maintainer_script::{Continuations, maintainer_script_lines};

/// lintian's `$LEADING_PATTERN`: where a shell command may begin.
const LEADING_PATTERN: &str = r#"(?:(?:^|[`&;(|{])\s*|(?:if|then|do|while|!)\s+|env(?:\s+[[:alnum:]_]+=(?:\S+|"[^"]*"|'[^']*'))*\s+)"#;

static DPKG_DIVERT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"{LEADING_PATTERN}(?:/usr/sbin/)?dpkg-divert\s")).expect("valid regex")
});
static NOT_A_DIVERSION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"--(?:help|list|truename|version)").expect("valid regex"));
static ARGUMENTS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"dpkg-divert\s*(.*)$").expect("valid regex"));
static OPTION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\s*(?:\$\{?[\w:=-]+\}?)*\s*--(?:add|quiet|remove|rename|no-rename|test|local|(?:admindir|divert|package)\s+\S+)\s*",
    )
    .expect("valid regex")
});
static PAREN_PAIR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^.*?\(.+?\)").expect("valid regex"));
static BRACE_PAIR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^.*?\{.+?\}").expect("valid regex"));
static COMMAND_END: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"&&|\|\||;|\d*>").expect("valid regex"));
static VARIABLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\\\$\w+").expect("valid regex"));
static BRACED_VARIABLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\\\$\\\{\w+.*?\\\}").expect("valid regex"));
static COMMAND_SUBSTITUTION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\\\$\\\(.+?\\\)").expect("valid regex"));
static MAN_PAGE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(usr\\/share\\/man\\/\S+\\/.*\\\.\d)\w*(\\\.gz\z)").expect("valid regex")
});

/// Whether a `dpkg-divert` call adds or removes the diversion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DiversionAction {
    Add,
    Remove,
}

/// One `dpkg-divert` call in a maintainer script.
#[derive(Clone, Debug)]
pub(crate) struct Diversion {
    script: PathBuf,
    line: u32,
    action: DiversionAction,
    local: bool,
    path: DivertedPath,
}

impl Diversion {
    /// The maintainer script, as a Control files path (`preinst`).
    pub fn script(&self) -> &Path {
        &self.script
    }

    /// 1-based last physical line of the call.
    pub fn line(&self) -> u32 {
        self.line
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn action(&self) -> DiversionAction {
        self.action
    }

    /// The line says `--local` anywhere.
    pub fn is_local(&self) -> bool {
        self.local
    }
}

/// A diverted path as lintian keys it: Perl-quoted, without the leading `/`,
/// with each shell expansion as an unquoted `.+`.
#[derive(Clone, Debug, PartialEq, Eq)]
struct DivertedPath(String);

impl DivertedPath {
    /// lintian's `unquote`: backslashes dropped, and in wildcard mode each
    /// `.+` shown as `*`.
    fn unquote(&self, wildcards: bool) -> String {
        let plain = self.0.replace('\\', "");
        if wildcards {
            plain.replace(".+", "*")
        } else {
            plain
        }
    }

    fn without_backslashes(&self) -> String {
        self.0.replace('\\', "")
    }

    /// `^path$` as a regex.
    fn matches_whole(&self, text: &str) -> bool {
        Regex::new(&format!("^(?:{})$", perl_quoted_to_regex(&self.0)))
            .is_ok_and(|regex| regex.is_match(text))
    }
}

/// Declared additions of one path, in declaration order of the path.
type Additions<'a> = Vec<(DivertedPath, &'a Diversion)>;

/// Declared removals of one path, in declaration order of the path.
type Removals<'a> = Vec<(DivertedPath, Vec<&'a Diversion>)>;

/// Diversions declared by the maintainer scripts of one Binary package.
#[derive(Debug, Default)]
pub(crate) struct Diversions {
    declared: Vec<Diversion>,
    /// A declared path has a shell expansion.
    wildcards: bool,
}

/// One `--remove` call after lintian's merge.
#[derive(Debug)]
pub(crate) struct RemovedDiversion {
    script: PathBuf,
    line: u32,
    path: DivertedPath,
    wildcards: bool,
    is_added: bool,
}

impl RemovedDiversion {
    /// The maintainer script of the call, as a Control files path.
    pub fn script(&self) -> &Path {
        &self.script
    }

    /// 1-based last physical line of the call.
    pub fn line(&self) -> u32 {
        self.line
    }

    /// The diverted path as lintian prints it, see [`AddedDiversion::path`].
    pub fn path(&self) -> String {
        self.path.unquote(self.wildcards)
    }

    /// An added diversion of the same path survives the merge.
    pub fn is_added(&self) -> bool {
        self.is_added
    }
}

/// An added diversion after lintian's merge: one per path, the last `--add`
/// of that path wins, and in wildcard mode a path takes the widest declared
/// pattern that matches it (added or removed).
#[derive(Debug)]
pub(crate) struct AddedDiversion {
    script: PathBuf,
    line: u32,
    path: DivertedPath,
    wildcards: bool,
    removed: bool,
}

impl AddedDiversion {
    /// The maintainer script of the winning `--add`.
    pub fn script(&self) -> &Path {
        &self.script
    }

    pub fn line(&self) -> u32 {
        self.line
    }

    /// Some `--remove` has the same path after the wildcard merge.
    pub fn is_removed(&self) -> bool {
        self.removed
    }

    /// The diverted path as lintian prints it: no leading `/`, each shell
    /// expansion as `*` in wildcard mode.
    pub fn path(&self) -> String {
        self.path.unquote(self.wildcards)
    }

    /// Whether the Data files have the diverted path.
    ///
    /// Outside wildcard mode that is the exact Entry name (a directory only
    /// with a trailing `/`). In wildcard mode the path is an unanchored regex
    /// over every Entry name, and a man page path matches every page of the
    /// same numeric section (`foo.1.gz` matches `foo.1p.gz`).
    pub fn is_in(&self, data: &FileTree) -> bool {
        let man_page = man_page_pattern(&self.path.0);
        if self.wildcards || man_page.is_some() {
            let quoted = man_page.unwrap_or_else(|| self.path.0.clone());
            let Ok(regex) = Regex::new(&perl_quoted_to_regex(&quoted)) else {
                return false;
            };
            return data
                .entries()
                .iter()
                .any(|entry| regex.is_match(&entry_name(entry)));
        }
        let name = self.path.unquote(false);
        if name.is_empty() {
            // lintian's lookup of the empty name is the root directory.
            return true;
        }
        data.get(name.trim_end_matches('/'))
            .is_some_and(|entry| entry_name(entry) == name)
    }
}

impl Diversions {
    pub fn from_control_files(files: &FileTree) -> Self {
        let mut diversions = Self::default();
        for line in maintainer_script_lines(files, Continuations::Always) {
            let Some((action, path, expanded)) = parse(&line.text) else {
                continue;
            };
            diversions.wildcards |= expanded;
            diversions.declared.push(Diversion {
                script: line.path,
                line: line.number,
                action,
                local: line.text.contains("--local"),
                path,
            });
        }
        diversions
    }

    /// Every `dpkg-divert` call, in Control files order then line order.
    pub fn declared(&self) -> &[Diversion] {
        &self.declared
    }

    /// Added diversions after lintian's merge, in declaration order of the
    /// surviving path.
    ///
    /// lintian iterates a hash, so a man page path switches the package to
    /// wildcard mode partway through at random. Here it does so for every
    /// added diversion.
    pub fn added(&self) -> Vec<AddedDiversion> {
        let (added, removed) = self.merged();
        let wildcards = self.wildcards
            || added
                .iter()
                .any(|(path, _)| man_page_pattern(&path.0).is_some());
        added
            .into_iter()
            .map(|(path, diversion)| {
                let removed = removed.iter().any(|(removed, _)| *removed == path);
                AddedDiversion {
                    script: diversion.script.clone(),
                    line: diversion.line,
                    path,
                    wildcards,
                    removed,
                }
            })
            .collect()
    }

    /// Every removal after lintian's merge, in declaration order within a
    /// path. `is_added` says whether an added diversion of the same path
    /// survives the merge.
    pub fn removed(&self) -> Vec<RemovedDiversion> {
        let (added, removed) = self.merged();
        let mut calls = Vec::new();
        for (path, diversions) in &removed {
            let is_added = added.iter().any(|(added, _)| added == path);
            for diversion in diversions {
                calls.push(RemovedDiversion {
                    script: diversion.script.clone(),
                    line: diversion.line,
                    path: path.clone(),
                    wildcards: self.wildcards,
                    is_added,
                });
            }
        }
        calls
    }

    /// lintian's merge of the declared calls into added and removed paths.
    fn merged(&self) -> (Additions<'_>, Removals<'_>) {
        let mut added: Additions = Vec::new();
        let mut removed: Removals = Vec::new();
        for diversion in &self.declared {
            match diversion.action {
                DiversionAction::Add => insert(&mut added, diversion.path.clone(), diversion),
                DiversionAction::Remove => {
                    match removed.iter_mut().find(|(path, _)| *path == diversion.path) {
                        Some((_, all)) => all.push(diversion),
                        None => removed.push((diversion.path.clone(), vec![diversion])),
                    }
                }
            }
        }
        if self.wildcards {
            widen(&mut added, &mut removed);
        }
        (added, removed)
    }
}

/// Set `key` to `value`, keeping the position of an existing key.
fn insert<V>(map: &mut Vec<(DivertedPath, V)>, key: DivertedPath, value: V) {
    match map.iter_mut().find(|(existing, _)| *existing == key) {
        Some((_, slot)) => *slot = value,
        None => map.push((key, value)),
    }
}

fn take<V>(map: &mut Vec<(DivertedPath, V)>, key: &DivertedPath) -> Option<V> {
    let index = map.iter().position(|(existing, _)| existing == key)?;
    Some(map.remove(index).1)
}

/// lintian's `installable` normalisation: every path that another declared
/// pattern matches moves to the widest such pattern. A moved entry replaces
/// one already at that pattern.
fn widen(
    added: &mut Vec<(DivertedPath, &Diversion)>,
    removed: &mut Vec<(DivertedPath, Vec<&Diversion>)>,
) {
    let keys = |added: &Vec<(DivertedPath, &Diversion)>,
                removed: &Vec<(DivertedPath, Vec<&Diversion>)>| {
        removed
            .iter()
            .map(|(path, _)| path.clone())
            .chain(added.iter().map(|(path, _)| path.clone()))
            .collect::<Vec<_>>()
    };
    for divert in keys(added, removed) {
        let present = added.iter().any(|(path, _)| *path == divert)
            || removed.iter().any(|(path, _)| *path == divert);
        if !present {
            continue;
        }
        let mut widest = divert.clone();
        let mut wider = widest.without_backslashes();
        let mut matches = Vec::new();
        for candidate in keys(added, removed) {
            let plain = candidate.without_backslashes();
            if candidate.matches_whole(&wider) {
                widest = candidate.clone();
                wider = plain;
                matches.push(candidate);
            } else if widest.matches_whole(&plain) {
                matches.push(candidate);
            }
        }
        for key in matches {
            if key == widest {
                continue;
            }
            if let Some(value) = take(removed, &key) {
                insert(removed, widest.clone(), value);
            }
            if let Some(value) = take(added, &key) {
                insert(added, widest.clone(), value);
            }
        }
    }
}

/// One code line: the action, the diverted path, and whether a shell
/// expansion was replaced. `None` when the line declares no diversion.
fn parse(line: &str) -> Option<(DiversionAction, DivertedPath, bool)> {
    if !DPKG_DIVERT.is_match(line) || NOT_A_DIVERSION.is_match(line) {
        return None;
    }
    let action = if line.contains("--remove") {
        DiversionAction::Remove
    } else {
        DiversionAction::Add
    };
    let arguments = ARGUMENTS.captures(line)?.get(1)?.as_str();
    let mut divert = OPTION.replace_all(arguments, NoExpand("")).into_owned();
    remove_unpaired(&mut divert, &PAREN_PAIR, ['(', ')']);
    remove_unpaired(&mut divert, &BRACE_PAIR, ['{', '}']);
    strip_command_end(&mut divert);
    divert.retain(|character| character != '"' && character != '\'');
    let divert = divert.strip_prefix('/').unwrap_or(&divert);
    let divert = divert.trim_matches(is_perl_space);

    let mut quoted = quotemeta(divert);
    let mut expanded = false;
    for pattern in [&*VARIABLE, &*BRACED_VARIABLE, &*COMMAND_SUBSTITUTION] {
        if pattern.is_match(&quoted) {
            quoted = pattern.replace_all(&quoted, NoExpand(".+")).into_owned();
            expanded = true;
        }
    }
    Some((action, DivertedPath(quoted), expanded))
}

/// lintian skips each balanced `open…close` from the start, then removes the
/// first `open` or `close` after the last one.
fn remove_unpaired(text: &mut String, pair: &Regex, [open, close]: [char; 2]) {
    let mut position = 0;
    while let Some(found) = pair.find(&text[position..]) {
        position += found.end();
    }
    if let Some(offset) = text[position..].find([open, close]) {
        text.remove(position + offset);
    }
}

/// Drop `&&`, `||`, `;`, or a redirection and the rest of the line, searching
/// after the last quoted string and at least one character in.
fn strip_command_end(text: &mut String) {
    let position = after_last_quoted(text);
    let Some(first) = text[position..].chars().next() else {
        return;
    };
    if let Some(found) = COMMAND_END.find_at(text, position + first.len_utf8()) {
        text.truncate(found.start());
    }
}

/// lintian's `1 while m/\G.*?(["']).+?\1/gc`: the offset after the last
/// `"…"` or `'…'` pair with at least one character inside, or 0.
fn after_last_quoted(text: &str) -> usize {
    let mut position = 0;
    'pairs: loop {
        for (offset, quote) in text[position..].char_indices() {
            if quote != '"' && quote != '\'' {
                continue;
            }
            let open = position + offset;
            let Some(inner) = text[open + 1..].chars().next() else {
                break 'pairs;
            };
            let search = open + 1 + inner.len_utf8();
            if let Some(close) = text[search..].find(quote) {
                position = search + close + 1;
                continue 'pairs;
            }
        }
        break;
    }
    position
}

fn is_perl_space(character: char) -> bool {
    matches!(character, ' ' | '\t' | '\n' | '\r' | '\x0b' | '\x0c')
}

/// Perl's `quotemeta`: a backslash before every character that is not an
/// ASCII word character.
fn quotemeta(text: &str) -> String {
    let mut quoted = String::with_capacity(text.len() * 2);
    for character in text.chars() {
        if !(character.is_ascii_alphanumeric() || character == '_') {
            quoted.push('\\');
        }
        quoted.push(character);
    }
    quoted
}

/// A Perl-quoted path as a Rust regex: each `\c` is the literal `c`; the
/// unquoted `.+` and `.*` stay patterns.
fn perl_quoted_to_regex(quoted: &str) -> String {
    let mut regex = String::with_capacity(quoted.len() * 2);
    let mut characters = quoted.chars();
    while let Some(character) = characters.next() {
        if character == '\\' {
            let literal = characters.next().unwrap_or('\\');
            regex.push_str(&regex::escape(literal.encode_utf8(&mut [0; 4])));
        } else {
            regex.push(character);
        }
    }
    regex
}

/// A man page path widened to every page of its numeric section.
fn man_page_pattern(quoted: &str) -> Option<String> {
    let captures = MAN_PAGE.captures(quoted)?;
    Some(format!("{}.*{}", &captures[1], &captures[2]))
}

/// lintian's Entry name: the path, with a trailing `/` for a directory.
fn entry_name(entry: &super::file_tree::Entry) -> String {
    let path = entry.path().to_string_lossy();
    if entry.kind() == EntryKind::Directory {
        format!("{path}/")
    } else {
        path.into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lint::file_tree::Entry;

    fn parsed(line: &str) -> Option<(DiversionAction, String, bool)> {
        parse(line).map(|(action, path, expanded)| (action, path.unquote(true), expanded))
    }

    fn scripts(files: &[(&str, &str)]) -> FileTree {
        FileTree::from_entries(
            None,
            files
                .iter()
                .map(|(path, content)| {
                    Entry::regular_file(*path, None, Some(content.as_bytes().to_vec()))
                })
                .collect(),
        )
    }

    fn data(paths: &[&str]) -> FileTree {
        FileTree::from_entries(
            None,
            paths
                .iter()
                .map(|path| match path.strip_suffix('/') {
                    Some(directory) => Entry::directory(directory),
                    None => Entry::regular_file(*path, None, Some(Vec::new())),
                })
                .collect(),
        )
    }

    #[test]
    fn options_and_their_arguments_are_dropped() {
        assert_eq!(
            parsed(
                "dpkg-divert --package foo --add --rename --divert /usr/bin/foo.real /usr/bin/foo"
            ),
            Some((DiversionAction::Add, "usr/bin/foo".to_string(), false))
        );
        assert_eq!(
            parsed("dpkg-divert $SOMETHING --package foo --remove --no-rename /usr/bin/foo"),
            Some((DiversionAction::Remove, "usr/bin/foo".to_string(), false))
        );
        assert_eq!(
            parsed("/usr/sbin/dpkg-divert --local --quiet --test --admindir /x \"/usr/bin/foo\""),
            Some((DiversionAction::Add, "usr/bin/foo".to_string(), false))
        );
    }

    #[test]
    fn non_diversions_are_skipped() {
        assert!(parsed("dpkg-divert --list '*foo'").is_none());
        assert!(parsed("x=$(dpkg-divert --truename /usr/bin/foo)").is_none());
        assert!(parsed("echo dpkg-divert --add /usr/bin/foo").is_none());
        assert!(parsed("dpkg-divert").is_none());
        assert!(parsed("if dpkg-divert --add /usr/bin/foo; then").is_some());
    }

    #[test]
    fn command_ends_and_unpaired_brackets_are_dropped() {
        let path = |line| parsed(line).map(|(_, path, _)| path);
        assert_eq!(
            path("dpkg-divert --add /usr/share/foo/bar > /dev/null"),
            Some("usr/share/foo/bar".to_string())
        );
        assert_eq!(
            path("dpkg-divert --add /usr/share/foo/bar 2> /dev/null"),
            Some("usr/share/foo/bar".to_string())
        );
        assert_eq!(
            path("dpkg-divert --add /etc/foo|| true"),
            Some("etc/foo".to_string())
        );
        assert_eq!(
            path("dpkg-divert --add /etc/foo; echo hi"),
            Some("etc/foo".to_string())
        );
        assert_eq!(
            path("(dpkg-divert --package foo --remove /etc/scripts/orphan)"),
            Some("etc/scripts/orphan".to_string())
        );
        assert_eq!(
            path("{ dpkg-divert --package foo --remove /usr/share/foo/$(echo bar); }"),
            Some("usr/share/foo/*".to_string())
        );
        // A separator inside the last quoted string stays.
        assert_eq!(
            path("dpkg-divert --add '/usr/share/a;b'"),
            Some("usr/share/a;b".to_string())
        );
    }

    #[test]
    fn shell_expansions_become_wildcards() {
        assert_eq!(
            parsed("dpkg-divert --add /usr/share/t1/\"$f\""),
            Some((DiversionAction::Add, "usr/share/t1/*".to_string(), true))
        );
        assert_eq!(
            parsed("dpkg-divert --add /usr/share/foo/${f#f}"),
            Some((DiversionAction::Add, "usr/share/foo/*".to_string(), true))
        );
        assert_eq!(
            parsed("dpkg-divert --remove /usr/lib/$d/old-file && echo"),
            Some((
                DiversionAction::Remove,
                "usr/lib/*/old-file".to_string(),
                true
            ))
        );
    }

    #[test]
    fn declared_follows_scripts_and_joins_continuations_everywhere() {
        let files = scripts(&[
            (
                "preinst",
                "#!/usr/bin/perl\n`dpkg-divert --local --add \\\n  /usr/bin/foo`;\n",
            ),
            (
                "postrm",
                "#!/bin/sh\n# dpkg-divert --remove /x\ndpkg-divert --remove /usr/bin/foo\n",
            ),
            ("postinst", "dpkg-divert --add /usr/bin/bar\n"),
        ]);
        let diversions = Diversions::from_control_files(&files);
        let declared: Vec<_> = diversions
            .declared()
            .iter()
            .map(|diversion| {
                (
                    diversion.script().display().to_string(),
                    diversion.line(),
                    diversion.action(),
                    diversion.is_local(),
                )
            })
            .collect();
        assert_eq!(
            declared,
            vec![
                ("postrm".to_string(), 3, DiversionAction::Remove, false),
                ("preinst".to_string(), 3, DiversionAction::Add, true),
            ]
        );
    }

    #[test]
    fn last_add_of_a_path_wins() {
        let files = scripts(&[(
            "preinst",
            "#!/bin/sh\ndpkg-divert --add /usr/bin/foo\ndpkg-divert --add /usr/bin/foo > /dev/null\n",
        )]);
        let added = Diversions::from_control_files(&files).added();
        assert_eq!(added.len(), 1);
        assert_eq!(added[0].line(), 3);
        assert_eq!(added[0].path(), "usr/bin/foo");
    }

    #[test]
    fn wildcard_mode_widens_to_the_declared_pattern() {
        let files = scripts(&[
            (
                "preinst",
                "#!/bin/sh\ndpkg-divert --add /etc/scripts/orphan\n",
            ),
            (
                "postrm",
                "#!/bin/sh\ndpkg-divert --remove /etc/scripts/$f\n",
            ),
        ]);
        let added = Diversions::from_control_files(&files).added();
        assert_eq!(added.len(), 1);
        assert_eq!(added[0].path(), "etc/scripts/*");
        assert_eq!(added[0].script(), Path::new("preinst"));
        assert_eq!(added[0].line(), 2);
    }

    #[test]
    fn exact_lookup_outside_wildcard_mode() {
        let tree = data(&["usr/", "usr/bin/", "usr/bin/foo", "usr/share/doc/"]);
        let lookup = |script: &str| {
            let files = scripts(&[("preinst", script)]);
            Diversions::from_control_files(&files).added()[0].is_in(&tree)
        };
        assert!(lookup("#!/bin/sh\ndpkg-divert --add /usr/bin/foo\n"));
        assert!(!lookup("#!/bin/sh\ndpkg-divert --add /usr/bin/fo\n"));
        assert!(!lookup("#!/bin/sh\ndpkg-divert --add /usr/share/doc\n"));
        assert!(lookup("#!/bin/sh\ndpkg-divert --add /usr/share/doc/\n"));
        assert!(!lookup("#!/bin/sh\ndpkg-divert --add /usr//bin/foo\n"));
    }

    #[test]
    fn wildcard_lookup_is_an_unanchored_regex() {
        let tree = data(&["usr/", "usr/share/", "usr/share/scripts/t1/orphan"]);
        let lookup = |script: &str| {
            let files = scripts(&[("preinst", script)]);
            Diversions::from_control_files(&files)
                .added()
                .iter()
                .map(|added| (added.path(), added.is_in(&tree)))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            lookup("#!/bin/sh\ndpkg-divert --add /usr/share/scripts/t1/$f\n"),
            vec![("usr/share/scripts/t1/*".to_string(), true)]
        );
        assert_eq!(
            lookup("#!/bin/sh\ndpkg-divert --add /etc/$f\ndpkg-divert --add /share/scripts\n"),
            vec![
                ("etc/*".to_string(), false),
                ("share/scripts".to_string(), true)
            ]
        );
    }

    #[test]
    fn man_page_matches_its_numeric_section() {
        let tree = data(&["usr/share/man/man1/foo.1p.gz", "usr/bin/foobar"]);
        let files = scripts(&[(
            "preinst",
            "#!/bin/sh\ndpkg-divert --add /usr/share/man/man1/foo.1.gz\ndpkg-divert --add /usr/bin/foo\n",
        )]);
        let added = Diversions::from_control_files(&files).added();
        assert!(added.iter().all(|added| added.is_in(&tree)));
    }
}
