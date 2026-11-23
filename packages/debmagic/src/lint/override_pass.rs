//! The override pass of ADR 0057: matches one processable's Lintian override
//! file against that processable's Diagnostics, as the per-processable loop of
//! lintian's `Lintian::Group::process` does.
//!
//! The pass is pure. It takes the parsed file, the facts about the processable
//! and its Diagnostics; it sets [`Diagnostic::overridden`] on the Diagnostics an
//! override suppressed and reports the overrides nothing matched. It is not
//! called yet: `run.rs` wires it in a later task.
#![allow(dead_code)]

use std::collections::{HashMap, HashSet};

use super::architecture::restriction_list_keeps;
use super::diagnostic::{Diagnostic, Overridden};
use super::lintian_overrides::LintianOverrides;
use super::lintian_tags::{current_tag_name, is_durable};
use super::rule::Tag;

/// What the pass must know about the processable besides its Diagnostics.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ProcessableFacts<'a> {
    /// Lintian's `Processable::architecture`: `source` for a Source tree or
    /// Source package, the control `Architecture` of an installable.
    pub architecture: &'a str,
    /// The Tags of the Rules that ran on this processable. A Rule that did not
    /// run (not selected, not implemented, not applicable) makes no override
    /// take part.
    pub ran_tags: &'a [Tag],
}

/// An override that took part and suppressed at least one Diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MatchedOverride {
    /// The current Tag name.
    pub tag: &'static str,
    /// The context pattern as written, whitespace collapsed; empty for none.
    pub pattern: String,
    /// 1-based line in the override file.
    pub line: u32,
    /// How many Diagnostics this override suppressed.
    pub hits: usize,
}

/// An override that took part and suppressed no Diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UnmatchedOverride {
    /// The Tag as written in the file, which may be an old name.
    pub tag_as_written: String,
    /// The context pattern as written, empty for none.
    pub pattern: String,
    /// 1-based line in the override file.
    pub line: u32,
}

/// What the pass did to one processable.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct PassOutcome {
    /// Overrides that suppressed at least one Diagnostic, in file order.
    pub matched: Vec<MatchedOverride>,
    /// Overrides whose Tag produced no Diagnostic here: `unused-override`.
    pub unused: Vec<UnmatchedOverride>,
    /// Overrides whose Tag produced Diagnostics here, none matching:
    /// `mismatched-override`.
    pub mismatched: Vec<UnmatchedOverride>,
    /// Overrides of a Show-Always Tag, which cannot be overridden and are
    /// ignored (lintian's `ignored_overrides`).
    pub ignored_durable: usize,
}

/// An override that takes part, keyed by (current Tag, pattern).
#[derive(Debug, Clone)]
struct Kept {
    tag: &'static str,
    tag_as_written: String,
    pattern: String,
    line: u32,
    justification: String,
    hits: usize,
}

/// Applies the override file to one processable's Diagnostics, in place.
///
/// Follows `Lintian::Group::process`: an override takes part when its Tag
/// resolves to a current name, a Rule with that Tag ran here, and its
/// architecture list keeps it; a Show-Always Tag is then ignored. A later line
/// with the same Tag and pattern replaces the earlier one. A Diagnostic is
/// matched by the first override of its Tag that is empty (which wins) or whose
/// pattern globs its [`Diagnostic::override_context`], shortest pattern first,
/// file order on ties.
pub(crate) fn apply(
    overrides: &LintianOverrides,
    facts: &ProcessableFacts<'_>,
    diagnostics: &mut [Diagnostic],
) -> PassOutcome {
    let ran: HashSet<&str> = facts.ran_tags.iter().map(|tag| tag.0).collect();
    let mut outcome = PassOutcome::default();
    let mut kept: Vec<Kept> = Vec::new();

    for item in overrides.overrides() {
        let Some(tag) = current_tag_name(item.tag()) else {
            continue;
        };
        if !ran.contains(tag) || !restriction_list_keeps(item.architectures(), facts.architecture) {
            continue;
        }
        if is_durable(tag) {
            outcome.ignored_durable += 1;
            continue;
        }
        kept.retain(|earlier| !(earlier.tag == tag && earlier.pattern == item.context()));
        kept.push(Kept {
            tag,
            tag_as_written: item.tag().to_string(),
            pattern: item.context().to_string(),
            line: item.line(),
            justification: item.justification().to_string(),
            hits: 0,
        });
    }

    // Per Tag, the kept overrides in matching order. The empty pattern has length
    // zero, so it sorts first and wins; the sort is stable, so ties keep file order.
    let globs: Vec<Vec<Token>> = kept.iter().map(|entry| compile(&entry.pattern)).collect();
    let mut by_tag: HashMap<&'static str, Vec<usize>> = HashMap::new();
    for (index, entry) in kept.iter().enumerate() {
        by_tag.entry(entry.tag).or_default().push(index);
    }
    for indices in by_tag.values_mut() {
        indices.sort_by_key(|&index| kept[index].pattern.chars().count());
    }

    let produced: HashSet<&'static str> = diagnostics.iter().map(|d| d.tag.0).collect();

    for diagnostic in diagnostics.iter_mut() {
        let Some(indices) = by_tag.get(diagnostic.tag.0) else {
            continue;
        };
        let context: Vec<char> = diagnostic.override_context().chars().collect();
        let Some(&index) = indices.iter().find(|&&index| {
            kept[index].pattern.is_empty() || glob_matches(&globs[index], &context)
        }) else {
            continue;
        };
        kept[index].hits += 1;
        diagnostic.overridden = Some(Overridden {
            path: overrides.path().to_path_buf(),
            line: kept[index].line,
            justification: kept[index].justification.clone(),
        });
    }

    for entry in kept {
        if entry.hits > 0 {
            outcome.matched.push(MatchedOverride {
                tag: entry.tag,
                pattern: entry.pattern,
                line: entry.line,
                hits: entry.hits,
            });
            continue;
        }
        let unmatched = UnmatchedOverride {
            tag_as_written: entry.tag_as_written,
            pattern: entry.pattern,
            line: entry.line,
        };
        if produced.contains(entry.tag) {
            outcome.mismatched.push(unmatched);
        } else {
            outcome.unused.push(unmatched);
        }
    }
    outcome
}

/// One token of an override pattern, as lintian's `match_glob` reads it:
/// `Regexp::Wildcards` in `jokers` mode, anchored at both ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Token {
    Literal(char),
    /// `?`: one character, but not a newline (`.` in the regex).
    AnyChar,
    /// `*`: any run of characters, but no newline.
    AnyRun,
}

/// Compiles an override pattern. A leading `./` is removed. A backslash escapes
/// the next character when that character is neither a word character nor
/// whitespace (so `\*`, `\?`, `\\`, `\-` are literal); otherwise the backslash is
/// literal, as `\a` and a trailing `\` are in lintian.
fn compile(pattern: &str) -> Vec<Token> {
    let pattern = pattern.strip_prefix("./").unwrap_or(pattern);
    let mut tokens = Vec::with_capacity(pattern.len());
    let mut chars = pattern.chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '\\' => match chars.peek().copied() {
                Some(next) if !is_word(next) && !next.is_whitespace() => {
                    chars.next();
                    tokens.push(Token::Literal(next));
                }
                _ => tokens.push(Token::Literal('\\')),
            },
            '?' => tokens.push(Token::AnyChar),
            '*' if tokens.last() == Some(&Token::AnyRun) => {}
            '*' => tokens.push(Token::AnyRun),
            other => tokens.push(Token::Literal(other)),
        }
    }
    tokens
}

fn is_word(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

/// Whether the whole `text` matches the compiled pattern. Iterative matching
/// with one backtrack point for the last `*`: a `*` that takes one more character
/// may not take a newline.
fn glob_matches(tokens: &[Token], text: &[char]) -> bool {
    let (mut p, mut t) = (0, 0);
    // The pattern index after the last `*`, and the text index where it ends now.
    let mut star: Option<(usize, usize)> = None;
    loop {
        if p < tokens.len() {
            match tokens[p] {
                Token::AnyRun => {
                    star = Some((p + 1, t));
                    p += 1;
                    continue;
                }
                Token::AnyChar if t < text.len() && text[t] != '\n' => {
                    p += 1;
                    t += 1;
                    continue;
                }
                Token::Literal(literal) if t < text.len() && text[t] == literal => {
                    p += 1;
                    t += 1;
                    continue;
                }
                _ => {}
            }
        } else if t == text.len() {
            return true;
        }
        match star {
            Some((resume, taken)) if taken < text.len() && text[taken] != '\n' => {
                star = Some((resume, taken + 1));
                p = resume;
                t = taken + 1;
            }
            _ => return false,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use serde::Deserialize;

    use super::*;
    use crate::lint::diagnostic::{Location, ProcessableType};

    /// `(pattern, context, matches)` from lintian 2.144.0's
    /// `Lintian::Util::match_glob`, run with `perl -I lib` over the pinned clone.
    #[derive(Deserialize)]
    struct GlobCase(String, String, bool);

    const RAN: &[Tag] = &[
        Tag("cute-field"),
        Tag("package-installs-apt-sources"),
        Tag("unused-override"),
        Tag("malformed-override"),
    ];

    fn facts(architecture: &str) -> ProcessableFacts<'_> {
        ProcessableFacts {
            architecture,
            ran_tags: RAN,
        }
    }

    fn overrides(text: &str) -> LintianOverrides {
        LintianOverrides::parse(
            PathBuf::from("usr/share/lintian/overrides/foo"),
            text,
            "foo",
            ProcessableType::Binary,
        )
    }

    fn diagnostic(tag: &'static str, message: &str) -> Diagnostic {
        Diagnostic::warn(message).with_tag(Tag(tag))
    }

    fn at(tag: &'static str, message: &str, path: &str, line: u32) -> Diagnostic {
        diagnostic(tag, message).with_location(Location {
            path: PathBuf::from(path),
            line: Some(line),
            column: None,
        })
    }

    fn glob(pattern: &str, context: &str) -> bool {
        let text: Vec<char> = context.chars().collect();
        glob_matches(&compile(pattern), &text)
    }

    fn hit_count(diagnostics: &[Diagnostic]) -> usize {
        diagnostics.iter().filter(|d| d.is_overridden()).count()
    }

    #[test]
    fn lintian_glob_table() {
        let text = include_str!("data/override-pass/glob-cases.jsonl");
        let cases: Vec<GlobCase> = text
            .lines()
            .map(|line| serde_json::from_str(line).expect("valid case line"))
            .collect();
        assert_eq!(cases.len(), 561);
        for GlobCase(pattern, context, expected) in cases {
            assert_eq!(
                glob(&pattern, &context),
                expected,
                "pattern {pattern:?} on context {context:?}"
            );
        }
    }

    #[test]
    fn escapes_are_literal() {
        assert!(glob(r"a\*", "a*"));
        assert!(!glob(r"a\*", "ab"));
        assert!(glob(r"a\?", "a?"));
        assert!(!glob(r"a\?", "ab"));
        assert!(glob(r"a\\b", r"a\b"));
        assert!(glob(r"a\.b", "a.b"));
        assert!(!glob(r"a\.b", "axb"));
        // A backslash before a letter is itself literal.
        assert!(glob(r"\a", r"\a"));
    }

    #[test]
    fn leading_dot_slash_is_dropped() {
        assert!(glob("./usr/*", "usr/bin/x"));
        assert!(glob("./*", "anything"));
        assert!(!glob("./usr/*", "./usr/bin/x"));
    }

    #[test]
    fn wildcards_do_not_cross_newline() {
        assert!(!glob("a*b", "a\nb"));
        assert!(!glob("a?b", "a\nb"));
        assert!(glob("a*b", "axxb"));
    }

    #[test]
    fn empty_pattern_suppresses_every_diagnostic_of_its_tag() {
        let parsed = overrides("cute-field\n");
        let mut diagnostics = vec![
            diagnostic("cute-field", "one"),
            diagnostic("cute-field", "two"),
            diagnostic("package-installs-apt-sources", "three"),
        ];
        let outcome = apply(&parsed, &facts("amd64"), &mut diagnostics);
        assert!(diagnostics[0].is_overridden());
        assert!(diagnostics[1].is_overridden());
        assert!(!diagnostics[2].is_overridden());
        assert_eq!(
            outcome.matched,
            [MatchedOverride {
                tag: "cute-field",
                pattern: String::new(),
                line: 1,
                hits: 2,
            }]
        );
        assert!(outcome.unused.is_empty());
        assert!(outcome.mismatched.is_empty());
    }

    #[test]
    fn overridden_carries_file_line_and_justification() {
        let parsed = overrides("# the reason\n# spread\ncute-field\n");
        let mut diagnostics = vec![diagnostic("cute-field", "x")];
        apply(&parsed, &facts("amd64"), &mut diagnostics);
        let overridden = diagnostics[0].overridden().expect("overridden");
        assert_eq!(
            overridden.path,
            Path::new("usr/share/lintian/overrides/foo")
        );
        assert_eq!(overridden.line, 3);
        assert_eq!(overridden.justification, "the reason spread");
    }

    #[test]
    fn pattern_matches_message_and_pointer() {
        let parsed = overrides("cute-field Field * [debian/control:5]\n");
        let mut diagnostics = vec![
            at("cute-field", "Field x", "debian/control", 5),
            at("cute-field", "Field x", "debian/control", 6),
            at("cute-field", "Field x", "debian/rules", 5),
        ];
        let outcome = apply(&parsed, &facts("amd64"), &mut diagnostics);
        let flags: Vec<bool> = diagnostics.iter().map(Diagnostic::is_overridden).collect();
        assert_eq!(flags, [true, false, false]);
        assert_eq!(outcome.matched.len(), 1);
        assert_eq!(outcome.matched[0].hits, 1);
        assert!(outcome.unused.is_empty());
        assert!(outcome.mismatched.is_empty());
    }

    #[test]
    fn pointer_with_line_does_not_match_a_pattern_without_it() {
        let parsed = overrides("cute-field * [debian/control]\n");
        let mut diagnostics = vec![
            at("cute-field", "x", "debian/control", 5),
            diagnostic("cute-field", "x"),
        ];
        apply(&parsed, &facts("amd64"), &mut diagnostics);
        // The first has a line, so its context is `x [debian/control:5]`: no match.
        // The second has no path, so it has no pointer at all: no match.
        assert!(!diagnostics[0].is_overridden());
        assert!(!diagnostics[1].is_overridden());
    }

    #[test]
    fn shortest_pattern_wins_ties_keep_file_order() {
        let parsed = overrides("cute-field *x*\ncute-field a*\ncute-field *\n");
        let mut diagnostics = vec![diagnostic("cute-field", "axb")];
        let outcome = apply(&parsed, &facts("amd64"), &mut diagnostics);
        assert_eq!(outcome.matched.len(), 1);
        assert_eq!(outcome.matched[0].pattern, "*");

        let parsed = overrides("cute-field *b\ncute-field a*\n");
        let mut diagnostics = vec![diagnostic("cute-field", "ab")];
        let outcome = apply(&parsed, &facts("amd64"), &mut diagnostics);
        assert_eq!(outcome.matched[0].pattern, "*b");
        assert_eq!(outcome.matched[0].line, 1);
        assert_eq!(outcome.unused.len(), 0);
        assert_eq!(outcome.mismatched.len(), 1);
    }

    #[test]
    fn empty_pattern_wins_over_other_patterns() {
        let parsed = overrides("cute-field a\ncute-field\n");
        let mut diagnostics = vec![diagnostic("cute-field", "a")];
        let outcome = apply(&parsed, &facts("amd64"), &mut diagnostics);
        assert_eq!(outcome.matched.len(), 1);
        assert_eq!(outcome.matched[0].pattern, "");
        assert_eq!(outcome.matched[0].line, 2);
        assert_eq!(outcome.mismatched[0].pattern, "a");
    }

    #[test]
    fn unused_and_mismatched_are_split_by_whether_the_tag_produced() {
        let parsed = overrides("cute-field nope\npackage-installs-apt-sources\n");
        let mut diagnostics = vec![diagnostic("cute-field", "x")];
        let outcome = apply(&parsed, &facts("amd64"), &mut diagnostics);
        assert!(outcome.matched.is_empty());
        assert_eq!(
            outcome.mismatched,
            [UnmatchedOverride {
                tag_as_written: "cute-field".to_string(),
                pattern: "nope".to_string(),
                line: 1,
            }]
        );
        assert_eq!(
            outcome.unused,
            [UnmatchedOverride {
                tag_as_written: "package-installs-apt-sources".to_string(),
                pattern: String::new(),
                line: 2,
            }]
        );
    }

    #[test]
    fn unknown_tag_is_skipped_not_reported() {
        let parsed = overrides("no-such-tag\nno-such-tag a\n");
        let mut diagnostics = vec![];
        let outcome = apply(&parsed, &facts("amd64"), &mut diagnostics);
        assert_eq!(outcome, PassOutcome::default());
    }

    #[test]
    fn tag_that_did_not_run_takes_no_part() {
        let parsed = overrides("cute-field\ncute-field nope\n");
        let facts = ProcessableFacts {
            architecture: "amd64",
            ran_tags: &[],
        };
        let mut diagnostics = vec![];
        let outcome = apply(&parsed, &facts, &mut diagnostics);
        // A known Tag no Rule ran here: neither applied nor reported as unused.
        assert_eq!(outcome, PassOutcome::default());
    }

    #[test]
    fn old_name_maps_to_current_and_is_reported_as_written() {
        let parsed = overrides("package-install-apt-sources\n");
        let mut diagnostics = vec![diagnostic("package-installs-apt-sources", "x")];
        let outcome = apply(&parsed, &facts("amd64"), &mut diagnostics);
        assert!(diagnostics[0].is_overridden());
        assert_eq!(outcome.matched[0].tag, "package-installs-apt-sources");

        let parsed = overrides("package-install-apt-sources\n");
        let mut none = vec![];
        let outcome = apply(&parsed, &facts("amd64"), &mut none);
        assert_eq!(
            outcome.unused[0].tag_as_written,
            "package-install-apt-sources"
        );
    }

    #[test]
    fn architecture_list_decides_whether_an_override_takes_part() {
        let text = "[amd64]: cute-field\n[arm64]: cute-field a\n[!amd64]: cute-field b\n";
        let parsed = overrides(text);
        let mut diagnostics = vec![diagnostic("cute-field", "a")];
        let outcome = apply(&parsed, &facts("amd64"), &mut diagnostics);
        assert!(diagnostics[0].is_overridden());
        assert_eq!(outcome.matched.len(), 1);
        assert_eq!(outcome.matched[0].line, 1);
        // The arm64 and negated lines are dropped, not unused.
        assert!(outcome.unused.is_empty());
        assert!(outcome.mismatched.is_empty());

        // On arm64 the `[amd64]` line drops out and the negated line takes part.
        let mut diagnostics = vec![diagnostic("cute-field", "b"), diagnostic("cute-field", "a")];
        let outcome = apply(&parsed, &facts("arm64"), &mut diagnostics);
        assert!(diagnostics[0].is_overridden());
        assert_eq!(diagnostics[0].overridden().map(|o| o.line), Some(3));
        assert_eq!(diagnostics[1].overridden().map(|o| o.line), Some(2));
        assert_eq!(outcome.matched.len(), 2);
        assert!(outcome.unused.is_empty());
        assert!(outcome.mismatched.is_empty());
    }

    #[test]
    fn source_tree_architecture_is_source() {
        let parsed = overrides("[source]: cute-field\n[amd64]: package-installs-apt-sources\n");
        let mut diagnostics = vec![
            diagnostic("cute-field", "x"),
            diagnostic("package-installs-apt-sources", "y"),
        ];
        let outcome = apply(&parsed, &facts("source"), &mut diagnostics);
        assert!(diagnostics[0].is_overridden());
        assert!(!diagnostics[1].is_overridden());
        assert!(outcome.unused.is_empty());
    }

    #[test]
    fn durable_tags_are_ignored_and_counted() {
        let parsed = overrides("unused-override\nmalformed-override x\ncute-field\n");
        let mut diagnostics = vec![
            diagnostic("unused-override", "a"),
            diagnostic("cute-field", "b"),
        ];
        let outcome = apply(&parsed, &facts("amd64"), &mut diagnostics);
        assert_eq!(outcome.ignored_durable, 2);
        assert!(!diagnostics[0].is_overridden());
        assert!(diagnostics[1].is_overridden());
        assert!(outcome.unused.is_empty());
        assert!(outcome.mismatched.is_empty());
    }

    #[test]
    fn later_duplicate_replaces_the_earlier_line() {
        let parsed = overrides("# first\ncute-field a\n# second\ncute-field a\n");
        let mut diagnostics = vec![diagnostic("cute-field", "a")];
        let outcome = apply(&parsed, &facts("amd64"), &mut diagnostics);
        assert_eq!(outcome.matched.len(), 1);
        assert_eq!(outcome.matched[0].line, 4);
        let overridden = diagnostics[0].overridden().expect("overridden");
        assert_eq!(overridden.line, 4);
        assert_eq!(overridden.justification, "second");
    }

    #[test]
    fn a_duplicate_dropped_by_architecture_does_not_replace() {
        let parsed = overrides("cute-field a\n[arm64]: cute-field a\n");
        let mut diagnostics = vec![diagnostic("cute-field", "a")];
        let outcome = apply(&parsed, &facts("amd64"), &mut diagnostics);
        assert_eq!(outcome.matched[0].line, 1);
    }

    #[test]
    fn unmatched_diagnostics_are_not_flagged() {
        let parsed = overrides("cute-field nope\n");
        let mut diagnostics = vec![
            diagnostic("cute-field", "x"),
            diagnostic("package-installs-apt-sources", "y"),
        ];
        apply(&parsed, &facts("amd64"), &mut diagnostics);
        assert_eq!(hit_count(&diagnostics), 0);
    }

    #[test]
    fn empty_file_changes_nothing() {
        let parsed = overrides("# only a comment\n\n");
        let mut diagnostics = vec![diagnostic("cute-field", "x")];
        let outcome = apply(&parsed, &facts("amd64"), &mut diagnostics);
        assert_eq!(outcome, PassOutcome::default());
        assert_eq!(hit_count(&diagnostics), 0);
    }
}
