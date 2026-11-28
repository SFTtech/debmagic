//! The spelling passes of lintian's `Lintian::Spelling`, shared by the Rules
//! that check text for misspellings: `check_spelling` over the vendored
//! `spelling/corrections` and `spelling/corrections-multiword`, and
//! `check_spelling_picky` over `spelling/corrections-case`.
//!
//! A caller passes the exceptions of the lintian group (see
//! [`group_exceptions`]) or none.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, OnceLock};

use fancy_regex::Regex as FancyRegex;
use regex::{Regex, RegexSet};

use crate::lint::context::BinaryPackageContext;
use crate::lint::data;

/// Words lintian's `spelling_exceptions` lists: never a misspelling, exact or
/// in lowercase.
pub(crate) type Exceptions = HashSet<String>;

type Pairs = HashMap<&'static str, &'static str>;

fn pairs(text: &'static str) -> Pairs {
    data::lines(text)
        .filter_map(|line| line.split_once("||"))
        .collect()
}

static CORRECTIONS: LazyLock<Pairs> =
    LazyLock::new(|| pairs(include_str!("data/spelling/corrections")));

static CORRECTIONS_CASE: LazyLock<Pairs> =
    LazyLock::new(|| pairs(include_str!("data/spelling/corrections-case")));

/// The `corrections-multiword` lines: `(pattern, correction)`, in file order.
static MULTIWORD_LINES: LazyLock<Vec<(&'static str, &'static str)>> = LazyLock::new(|| {
    data::lines(include_str!("data/spelling/corrections-multiword"))
        .filter_map(|line| line.split_once("||"))
        .collect()
});

/// How `multiword_candidates` finds the `MULTIWORD_LINES` that can match a text.
struct MultiwordSplit {
    /// The lines without lookaround, as one set over `\b(?:…)\b`.
    set: RegexSet,
    /// The line index of each `set` pattern.
    plain: Vec<usize>,
    /// The lines with lookaround (which `regex` cannot compile), each with a
    /// prefilter: a `regex` that matches wherever the line can match. `None`
    /// when the pattern cannot be relaxed, so the line is checked on every text.
    lookaround: Vec<(usize, Option<Regex>)>,
}

static MULTIWORD_SPLIT: LazyLock<MultiwordSplit> = LazyLock::new(|| {
    let (fancy, plain): (Vec<usize>, Vec<usize>) =
        (0..MULTIWORD_LINES.len()).partition(|&index| needs_fancy(MULTIWORD_LINES[index].0));
    let set = RegexSet::new(
        plain
            .iter()
            .map(|&index| format!(r"\b(?:{})\b", MULTIWORD_LINES[index].0)),
    )
    .expect("multiword pattern");
    let lookaround = fancy
        .into_iter()
        .map(|index| {
            let relaxed = without_lookaround(MULTIWORD_LINES[index].0)
                .and_then(|pattern| Regex::new(&format!(r"\b(?:{pattern})\b")).ok());
            (index, relaxed)
        })
        .collect();
    MultiwordSplit {
        set,
        plain,
        lookaround,
    }
});

/// Patterns that `regex` cannot compile (lookaround) need `fancy-regex`.
fn needs_fancy(pattern: &str) -> bool {
    ["(?=", "(?!", "(?<=", "(?<!"]
        .iter()
        .any(|look| pattern.contains(look))
}

/// `pattern` with its lookaround groups removed. A lookaround is zero-width, so
/// the result matches at every position where `pattern` matches, and the
/// first such position is a valid starting point for the confirming run.
/// `None` for a pattern this does not understand (nested character classes,
/// unbalanced groups), which then gets no prefilter.
fn without_lookaround(pattern: &str) -> Option<String> {
    const LOOKAROUND: [&str; 4] = ["(?=", "(?!", "(?<=", "(?<!"];
    let mut out = String::with_capacity(pattern.len());
    let mut rest = pattern;
    while let Some(at) = rest.find(['\\', '[', '(']) {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        if let Some(escaped) = tail.strip_prefix('\\') {
            let next = escaped.chars().next()?;
            out.push('\\');
            out.push(next);
            rest = &escaped[next.len_utf8()..];
        } else if tail.starts_with('[') {
            let end = class_end(tail)?;
            out.push_str(&tail[..end]);
            rest = &tail[end..];
        } else if LOOKAROUND.iter().any(|look| tail.starts_with(look)) {
            rest = &tail[group_end(tail)?..];
        } else {
            out.push('(');
            rest = &tail[1..];
        }
    }
    out.push_str(rest);
    Some(out)
}

/// The byte past the `]` closing the character class at the start of `text`.
fn class_end(text: &str) -> Option<usize> {
    let mut chars = text.char_indices().skip(1).peekable();
    if chars.peek().is_some_and(|&(_, c)| c == '^') {
        chars.next();
    }
    // A `]` first in the class is a literal.
    if chars.peek().is_some_and(|&(_, c)| c == ']') {
        chars.next();
    }
    while let Some((index, c)) = chars.next() {
        match c {
            ']' => return Some(index + 1),
            '[' => return None,
            '\\' => {
                chars.next()?;
            }
            _ => {}
        }
    }
    None
}

/// The byte past the `)` closing the group at the start of `text`.
fn group_end(text: &str) -> Option<usize> {
    let mut depth = 0usize;
    let mut at = 0;
    while let Some(c) = text[at..].chars().next() {
        match c {
            '\\' => at += 1 + text[at + 1..].chars().next()?.len_utf8(),
            '[' => at += class_end(&text[at..])?,
            '(' | ')' => {
                depth = if c == '(' {
                    depth + 1
                } else {
                    depth.checked_sub(1)?
                };
                at += 1;
                if depth == 0 {
                    return Some(at);
                }
            }
            _ => at += c.len_utf8(),
        }
    }
    None
}

/// The fancy-regex of `MULTIWORD_LINES[index]`, compiled on first use. It
/// captures the text of the pattern; `regex` only tells whether it matches.
/// `None` when the pattern does not compile.
///
/// lintian's Perl regexes have no step limit. `fancy-regex` defaults to one
/// and then fails on long texts, so the limit is lifted to match lintian.
fn multiword_regex(index: usize) -> Option<&'static FancyRegex> {
    static SLOTS: LazyLock<Vec<OnceLock<Option<FancyRegex>>>> =
        LazyLock::new(|| MULTIWORD_LINES.iter().map(|_| OnceLock::new()).collect());
    SLOTS[index]
        .get_or_init(|| {
            fancy_regex::RegexBuilder::new(&format!(r"\b({})\b", MULTIWORD_LINES[index].0))
                .backtrack_limit(usize::MAX)
                .build()
                .ok()
        })
        .as_ref()
}

/// The `MULTIWORD_LINES` that can match `text`, in file order, each with the
/// byte position its confirming fancy-regex run starts at. A line without a
/// prefilter match has no match at all, so it is not a candidate. Lines the
/// `RegexSet` reports start at 0; a lookaround line starts at the first
/// position of its relaxed pattern, which no real match precedes.
fn multiword_candidates(text: &str) -> Vec<(usize, usize)> {
    let split = &*MULTIWORD_SPLIT;
    let mut candidates: Vec<(usize, usize)> = split
        .set
        .matches(text)
        .into_iter()
        .map(|set_index| (split.plain[set_index], 0))
        .collect();
    for (index, relaxed) in &split.lookaround {
        match relaxed {
            Some(relaxed) => {
                if let Some(first) = relaxed.find(text) {
                    candidates.push((*index, first.start()));
                }
            }
            None => candidates.push((*index, 0)),
        }
    }
    candidates.sort_unstable();
    candidates
}

static META_PACKAGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"meta\s+package").expect("valid regex"));

static BRACKETED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)\[.+?\]").expect("valid regex"));

static HYPHEN_BREAK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\w-)\s*\n\s*").expect("valid regex"));

const TRAILING_PUNCTUATION: &[char] = &['.', ',', ';', ':', '?', '!'];

/// lintian's `check_spelling`: `(found, correction)` pairs for `text`, in the
/// order lintian reports them, each misspelled word once per lowercase form.
///
/// A repeated word is reported as `"<word> <word> (duplicate word)"` with the
/// word as correction, unless `duplicate_check` is off. Multi-word
/// misspellings are reported quoted, after the single words. Words in
/// `exceptions` are never reported, exact or lowercase; acronyms of one to
/// five capitals are never reported.
pub(crate) fn check_spelling(
    text: &str,
    exceptions: &Exceptions,
    duplicate_check: bool,
) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let normalized = normalize(text);
    let mut seen: HashSet<String> = HashSet::new();
    let mut duplicates: HashSet<&str> = HashSet::new();
    let mut last_word: Option<&str> = None;
    let mut quoted = false;
    let mut lower = String::new();

    for raw in normalized.split_whitespace() {
        let ends_with_punct = raw.ends_with(TRAILING_PUNCTUATION);
        let word = raw.trim_end_matches(TRAILING_PUNCTUATION);

        if duplicate_check {
            if word.matches('"').count() % 2 == 1 {
                quoted = !quoted;
            }
            if last_word == Some(word)
                && !quoted
                && duplicates.insert(word)
                && !ends_with_punct
                && !repeated_in_parens(text, word)
            {
                found.push((format!("{word} {word} (duplicate word)"), word.to_string()));
            }
            last_word = (is_ascii_alpha(word) && !ends_with_punct).then_some(word);
        }

        let Some(lcword) = lowercase_key(word, &mut lower) else {
            continue;
        };
        let Some(correction) = CORRECTIONS.get(lcword) else {
            continue;
        };
        if is_one_to_five_capitals(word) || exceptions.contains(word) || exceptions.contains(lcword)
        {
            continue;
        }
        let correction = adjust_case(word, correction);
        if seen.insert(lcword.to_string()) {
            found.push((word.to_string(), correction));
        }
    }

    for (index, start) in multiword_candidates(&normalized) {
        let Some(regex) = multiword_regex(index) else {
            continue;
        };
        let Some(matched) = regex
            .captures_from_pos(&normalized, start)
            .ok()
            .flatten()
            .and_then(|caps| caps.get(1))
        else {
            continue;
        };
        let matched = matched.as_str();
        let correction = adjust_case(matched, MULTIWORD_LINES[index].1);
        if seen.insert(matched.to_lowercase()) {
            found.push((format!("\"{matched}\""), format!("\"{correction}\"")));
        }
    }
    found
}

/// lintian's `check_spelling_picky`: `(found, correction)` pairs for the
/// capitalization errors of `text`. The `meta package` check comes first and
/// is not deduplicated. Text in square brackets is not checked.
pub(crate) fn check_spelling_picky(text: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    if META_PACKAGE.is_match(text) {
        found.push(("meta package".to_string(), "metapackage".to_string()));
    }
    // Text in square brackets may be a package list, so it is not checked.
    let stripped = BRACKETED.replace_all(text, "");
    let mut seen = HashSet::new();
    for word in stripped.split_whitespace() {
        let word = strip_word_edges(word);
        if let Some(correction) = CORRECTIONS_CASE.get(word)
            && seen.insert(word)
        {
            found.push((word.to_string(), (*correction).to_string()));
        }
    }
    found
}

/// Lintian's text preparation before the word loop: brackets are dropped,
/// parentheses are dropped except `(s)`, a hyphen at a line break is joined,
/// and whitespace runs become single spaces.
fn normalize(text: &str) -> String {
    let no_parens = strip_brackets_and_parentheses(text);
    // `(\w-)\s*\n\s*` needs a newline, so most texts skip the regex.
    let joined = if no_parens.contains('\n') {
        HYPHEN_BREAK.replacen(&no_parens, 1, "$1")
    } else {
        Cow::Borrowed(no_parens.as_str())
    };
    let mut out = String::with_capacity(joined.len());
    for word in joined.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    out
}

/// `s/[\[\]]//g`, then `s/(\((?!s\))|(?<!\(s)\))//gi` on the result, in one
/// pass: `(` is dropped unless `s)` follows it, and `)` is dropped unless `(s`
/// precedes it. Both look at the text with the brackets gone, as lintian does.
fn strip_brackets_and_parentheses(text: &str) -> String {
    let mut kept = text.chars().filter(|c| !matches!(c, '[' | ']'));
    let mut out = String::with_capacity(text.len());
    // The two kept characters before the current one, nearest first.
    let mut before: [Option<char>; 2] = [None, None];
    while let Some(c) = kept.next() {
        let keep = match c {
            '(' => {
                let mut ahead = kept.clone();
                ahead.next().is_some_and(|n| n.eq_ignore_ascii_case(&'s'))
                    && ahead.next() == Some(')')
            }
            ')' => {
                before[1] == Some('(') && before[0].is_some_and(|p| p.eq_ignore_ascii_case(&'s'))
            }
            _ => true,
        };
        if keep {
            out.push(c);
        }
        before = [Some(c), before[0]];
    }
    out
}

/// The lowercase form of `word` as a `CORRECTIONS` key would be written, in
/// `buf`. An ASCII word is lowered in place and skipped when no key can have
/// its length. Any other word takes Unicode lowercasing (`str::to_lowercase`,
/// which also maps a final `Σ` to `ς`), since a non-ASCII letter can lower to
/// an ASCII one (KELVIN SIGN to `k`). `None` when no key can match.
fn lowercase_key<'a>(word: &str, buf: &'a mut String) -> Option<&'a str> {
    if word.is_ascii() {
        let (min, max) = *CORRECTION_KEY_LENGTHS;
        if !(min..=max).contains(&word.len()) {
            return None;
        }
        buf.clear();
        buf.extend(word.chars().map(|c| c.to_ascii_lowercase()));
    } else {
        *buf = word.to_lowercase();
    }
    Some(buf.as_str())
}

/// The shortest and longest `CORRECTIONS` key, in bytes.
static CORRECTION_KEY_LENGTHS: LazyLock<(usize, usize)> = LazyLock::new(|| {
    CORRECTIONS.keys().fold((usize::MAX, 0), |(min, max), key| {
        (min.min(key.len()), max.max(key.len()))
    })
});

/// `\b<word>\s*\(<word>\b` or `\b<word>\)\s*<word>\b` in the original text:
/// the word is shown in parentheses, which is not a duplicate.
fn repeated_in_parens(original: &str, word: &str) -> bool {
    let word = regex::escape(word);
    [
        format!(r"\b{word}\s*\({word}\b"),
        format!(r"\b{word}\)\s*{word}\b"),
    ]
    .iter()
    .any(|pattern| Regex::new(pattern).is_ok_and(|regex| regex.is_match(original)))
}

/// `^[A-Z]+$` gives all capitals, `^[A-Z]` the first one.
fn adjust_case(word: &str, correction: &str) -> String {
    if !word.is_empty() && word.chars().all(|c| c.is_ascii_uppercase()) {
        correction.to_uppercase()
    } else if word.starts_with(|c: char| c.is_ascii_uppercase()) {
        let mut chars = correction.chars();
        match chars.next() {
            Some(first) => first.to_uppercase().chain(chars).collect(),
            None => String::new(),
        }
    } else {
        correction.to_string()
    }
}

fn is_ascii_alpha(word: &str) -> bool {
    !word.is_empty() && word.chars().all(|c| c.is_ascii_alphabetic())
}

/// `^[A-Z]{1,5}\z`: an acronym is not a misspelling.
fn is_one_to_five_capitals(word: &str) -> bool {
    (1..=5).contains(&word.len()) && word.chars().all(|c| c.is_ascii_uppercase())
}

/// Strips a leading `(` and trailing `).,?!:;` characters, as the picky check does.
fn strip_word_edges(word: &str) -> &str {
    let word = word.strip_prefix('(').unwrap_or(word);
    word.trim_end_matches([')', '.', ',', '?', '!', ':', ';'])
}

/// The words of `debian/<binary>` exceptions, `binaries/spelling-exceptions`:
/// the first field of each line.
#[allow(dead_code)] // seam API; no Rule reads it yet
pub(crate) fn binary_exception_words() -> impl Iterator<Item = &'static str> {
    data::lines(include_str!("data/binaries/spelling-exceptions"))
        .filter_map(|line| line.split_whitespace().next())
}

/// The spelling exceptions of a Binary package's group, as lintian's
/// `spelling_exceptions` builds them for one processable: the package and
/// source names with their `-` pieces, and the names of the people in
/// `Maintainer`, `Uploaders` and `Changed-By` with their words. The group
/// of a Binary package Subject is only the package, so the source's own
/// people fields are not read.
#[allow(dead_code)] // seam API; no Rule reads it yet
pub(crate) fn group_exceptions(ctx: &BinaryPackageContext<'_>) -> Exceptions {
    let source = ctx.source_name();
    let mut names = vec![ctx.package_name().to_string(), source];
    for name in names.clone() {
        names.extend(name.split('-').map(str::to_string));
    }
    let fields = ctx.installation_control().fields();
    let mut people = Vec::new();
    for field in ["Maintainer", "Uploaders", "Changed-By"] {
        let value = fields.get(field).unwrap_or_default();
        people.extend(person_names(value));
    }
    for person in people.clone() {
        names.extend(person.split_whitespace().map(str::to_string));
    }
    names.extend(people);
    names.into_iter().collect()
}

/// The spelling exceptions of a Source package's group, as far as its
/// `debian/control` tells: the source and binary names with their `-` pieces,
/// and the phrases of `Maintainer` and `Uploaders` with their words. lintian
/// also reads `Changed-By` and the people fields of the group's binary
/// packages, which a Source package Subject does not have, so those are not
/// exempt here.
pub(crate) fn source_exceptions(
    control: &crate::lint::debian_control::DebianControl,
) -> Exceptions {
    let source = control.source();
    let mut names: Vec<String> = Vec::new();
    if let Some(name) = source
        .get("Source")
        .and_then(|value| value.split_whitespace().next())
    {
        names.push(name.to_string());
    }
    names.extend(
        control
            .installables()
            .iter()
            .map(|installable| installable.name().to_string()),
    );
    for name in names.clone() {
        names.extend(name.split('-').map(str::to_string));
    }
    let mut people = Vec::new();
    for field in ["Maintainer", "Uploaders"] {
        let value = source.unfolded_value(field).unwrap_or_default();
        people.extend(person_names(&value));
    }
    for person in people.clone() {
        names.extend(person.split_whitespace().map(str::to_string));
    }
    names.extend(people);
    names.into_iter().collect()
}

/// The `phrase` of each `Name <address>` in a list of addresses.
fn person_names(value: &str) -> Vec<String> {
    value
        .split(',')
        .filter_map(|address| {
            let (phrase, _) = address.split_once('<')?;
            let phrase = phrase.trim().trim_matches('"').trim();
            (!phrase.is_empty()).then(|| phrase.to_string())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(text: &str) -> Vec<(String, String)> {
        check_spelling(text, &Exceptions::new(), true)
    }

    fn quoted(found: &str, correction: &str) -> (String, String) {
        (format!("\"{found}\""), format!("\"{correction}\""))
    }

    #[test]
    fn multiword_patterns_match_case_insensitively_on_word_boundaries() {
        assert_eq!(
            check_spelling("this is not enable yet", &Exceptions::new(), false),
            [quoted("is not enable", "is not enabled")]
        );
        assert_eq!(
            check_spelling("IS NOT ENABLE", &Exceptions::new(), false),
            [quoted("IS NOT ENABLE", "Is not enabled")]
        );
        assert_eq!(
            check_spelling("we have to be pass the tests", &Exceptions::new(), false),
            [quoted("have to be pass", "have to pass")]
        );
        assert_eq!(
            check_spelling("the debian/gnu linux system", &Exceptions::new(), false),
            [quoted("debian/gnu linux", "Debian GNU/Linux")]
        );
        // `(?i)none? the less`: the `?` makes the `e` optional.
        assert_eq!(
            check_spelling("none the less", &Exceptions::new(), false),
            [quoted("none the less", "nonetheless")]
        );
        // Not a word boundary: `allow to` inside `disallow to`.
        assert!(check_spelling("disallow to run", &Exceptions::new(), false).is_empty());
        assert!(check_spelling("allows tomorrow", &Exceptions::new(), false).is_empty());
    }

    #[test]
    fn multiword_lookahead_pattern_still_applies() {
        // `these package(?!\s+\w+s\b)` has lookaround, so it is checked on every text.
        assert_eq!(
            check_spelling("these package", &Exceptions::new(), false),
            [quoted("these package", "this package")]
        );
        assert!(check_spelling("these packages install", &Exceptions::new(), false).is_empty());
    }

    #[test]
    fn multiword_patterns_are_not_fooled_by_non_ascii_text() {
        assert_eq!(
            check_spelling("caf\u{e9} is not enable", &Exceptions::new(), false),
            [quoted("is not enable", "is not enabled")]
        );
        // LATIN SMALL LETTER LONG S folds to `s` under Unicode case folding.
        assert_eq!(
            check_spelling("\u{17f}eams like it", &Exceptions::new(), false),
            [quoted("\u{17f}eams like", "seems like")]
        );
        // `\b` is Unicode-aware: an accented letter is a word character.
        assert!(check_spelling("\u{e9}seams like it", &Exceptions::new(), false).is_empty());
    }

    #[test]
    fn multiword_hits_keep_file_order() {
        let text = "per say, over ridden, seams like";
        let hits: Vec<String> = check_spelling(text, &Exceptions::new(), false)
            .into_iter()
            .map(|(found, _)| found)
            .collect();
        assert_eq!(hits, ["\"over ridden\"", "\"per say\"", "\"seams like\""]);
    }

    #[test]
    fn spells_and_reports_duplicates() {
        assert_eq!(
            found("a mroe, mroe tool"),
            [("mroe".to_string(), "more".to_string())]
        );
        assert_eq!(
            found("these these tools"),
            [(
                "these these (duplicate word)".to_string(),
                "these".to_string()
            )]
        );
    }

    #[test]
    fn duplicate_check_can_be_off() {
        assert!(check_spelling("these these tools", &Exceptions::new(), false).is_empty());
    }

    #[test]
    fn keeps_acronyms_and_plural_s() {
        assert!(found("the USA (s) tool").is_empty());
    }

    #[test]
    fn honours_exceptions_exact_and_lowercase() {
        let exceptions: Exceptions = ["mroe".to_string()].into_iter().collect();
        assert!(check_spelling("a MROE tool", &exceptions, true).is_empty());
        assert!(check_spelling("a Mroe tool", &exceptions, true).is_empty());
    }

    #[test]
    fn adjusts_case_of_the_correction() {
        assert_eq!(found("Mroe"), [("Mroe".to_string(), "More".to_string())]);
        // Acronyms of up to five capitals are never reported.
        assert!(found("MROE").is_empty());
        assert_eq!(
            found("SPELING"),
            [("SPELING".to_string(), "SPELLING".to_string())]
        );
    }

    #[test]
    fn multiword_is_quoted() {
        assert_eq!(
            found("this packages are"),
            [(
                "\"this packages\"".to_string(),
                "\"these packages\"".to_string()
            )]
        );
    }

    #[test]
    fn lookahead_multiword_pattern_is_loaded() {
        assert!(found("these package tools").is_empty());
        assert_eq!(
            found("these package"),
            [(
                "\"these package\"".to_string(),
                "\"this package\"".to_string()
            )]
        );
    }

    #[test]
    fn picky_capitalization_once_per_word() {
        assert_eq!(
            check_spelling_picky("debian is debian, (debian)."),
            [("debian".to_string(), "Debian".to_string())]
        );
    }

    #[test]
    fn picky_skips_bracketed_text_and_reports_meta_package() {
        assert_eq!(
            check_spelling_picky("[debian] a meta package"),
            [("meta package".to_string(), "metapackage".to_string())]
        );
    }

    #[test]
    fn person_phrases() {
        assert_eq!(
            person_names("\"Ann Lee\" <a@x>, <b@x>, Cy <c@x>"),
            ["Ann Lee", "Cy"]
        );
    }

    #[test]
    fn source_exceptions_from_control() {
        let control = crate::lint::debian_control::DebianControl::parse(
            "Source: alot-tools\nMaintainer: Ann Lee <a@x>, Cy <c@x>\nUploaders: Dee <d@x>\n\n\
             Package: alot-tools-doc\nArchitecture: all\nDescription: x\n",
        )
        .expect("parses");
        let words = source_exceptions(&control);
        for word in [
            "alot-tools",
            "alot",
            "tools",
            "alot-tools-doc",
            "doc",
            "Ann Lee",
            "Ann",
            "Lee",
            "Cy",
            "Dee",
        ] {
            assert!(words.contains(word), "{word} is exempt");
        }
        assert!(!words.contains("speling"));
    }

    #[test]
    fn binary_exceptions_are_first_fields() {
        assert!(binary_exception_words().any(|word| word == "teH"));
    }

    /// lintian matches each multi-word pattern against the whole text with
    /// no step limit (`check_spelling`, `$text =~ $oregex`), so a phrase late
    /// in a long binary's strings is still reported.
    #[test]
    fn multiword_phrase_late_in_long_text_is_reported() {
        let mut text = "filler word here ".repeat(60_000);
        text.push_str(" The tool does allow to run.");
        let found = found(&text);
        assert!(
            found.contains(&("\"allow to\"".to_string(), "\"allow one to\"".to_string())),
            "{found:?}"
        );
    }

    /// lintian runs every multi-word pattern on its own, so `allows to` and
    /// `allow to` are each reported at their first match (`check_spelling`).
    #[test]
    fn multiword_patterns_each_report_their_first_match() {
        let text = "GtkX allows to show. They allow to select.";
        assert_eq!(
            found(text),
            [
                ("\"allows to\"".to_string(), "\"allows one to\"".to_string()),
                ("\"allow to\"".to_string(), "\"allow one to\"".to_string()),
            ]
        );
    }

    /// Boundary: a phrase is only reported on word boundaries, so `allow tomorrow`
    /// and `disallow to` are not reported (`\b...\b` in `check_spelling`).
    #[test]
    fn multiword_phrase_needs_word_boundaries() {
        assert!(found("We will allow tomorrow and disallow to stop.").is_empty());
    }

    /// A plain multi-word pattern is confirmed from 0, so its first match
    /// after a rejected `\b` candidate is the one reported.
    #[test]
    fn multiword_match_after_a_non_boundary_occurrence() {
        assert_eq!(
            found("disallow to run; allow to stop"),
            [quoted("allow to", "allow one to")]
        );
    }

    /// The lookaround line is confirmed from its prefilter's first position. The
    /// first `these package` is rejected by the lookahead (`tools` ends in s),
    /// so the confirming run goes on to the second one.
    #[test]
    fn lookaround_match_after_a_rejected_candidate_is_found() {
        assert_eq!(
            found("xx these package tools; yy these package x"),
            [quoted("these package", "this package")]
        );
        assert!(found("rethese package x").is_empty());
    }

    #[test]
    fn lookaround_candidate_starts_at_the_prefilter_position() {
        let index = MULTIWORD_LINES
            .iter()
            .position(|(pattern, _)| pattern.starts_with("(?i)these package("))
            .expect("line");
        let candidates = multiword_candidates("xx these package tools; yy these package x");
        assert!(candidates.contains(&(index, 3)), "{candidates:?}");
        assert!(!multiword_candidates("nothing here").contains(&(index, 0)));
    }

    #[test]
    fn lookaround_groups_are_removed_for_the_prefilter() {
        assert_eq!(
            without_lookaround(r"these package(?!\s+\w+s\b)").as_deref(),
            Some("these package")
        );
        assert_eq!(
            without_lookaround(r"a(?<=b)c[(?=]d").as_deref(),
            Some("ac[(?=]d")
        );
        assert_eq!(without_lookaround(r"x(?=a(b)c)y").as_deref(), Some("xy"));
        // A `)` inside a class does not close the group.
        assert_eq!(without_lookaround(r"x(?=[)]a)y").as_deref(), Some("xy"));
        assert_eq!(without_lookaround(r"a\(?=b").as_deref(), Some(r"a\(?=b"));
        assert_eq!(without_lookaround("[]a](?!b)").as_deref(), Some("[]a]"));
        assert_eq!(without_lookaround("[[a]]"), None);
        assert_eq!(without_lookaround("x(?=a"), None);
    }

    /// Non-ASCII words take Unicode lowercasing: a CYRILLIC letter is its own
    /// key, and a KELVIN SIGN lowers to the ASCII `k` of a key.
    #[test]
    fn non_ascii_words_are_lowercased_as_unicode() {
        assert_eq!(
            found("a \u{441}ontain b"),
            [("\u{441}ontain".to_string(), "contain".to_string())]
        );
        assert_eq!(
            found("a \u{421}ontain b"),
            [("\u{421}ontain".to_string(), "contain".to_string())]
        );
        assert_eq!(
            found("\u{212a}enrel tool"),
            [("\u{212a}enrel".to_string(), "kernel".to_string())]
        );
    }

    #[test]
    fn normalize_drops_brackets_and_parentheses_except_s() {
        assert_eq!(normalize("a [b] (c) (s) x(s)y"), "a b c (s) x(s)y");
        // The brackets go first, so `(s)` around a bracket is kept.
        assert_eq!(normalize("([s])"), "(s)");
    }

    #[test]
    fn normalize_joins_only_the_first_hyphen_break() {
        assert_eq!(normalize("foo-\n   bar"), "foo-bar");
        assert_eq!(normalize("a-\nb c-\nd"), "a-b c- d");
        assert_eq!(normalize("  one \t two\n\nthree  "), "one two three");
    }
}
