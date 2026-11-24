//! The spelling passes of lintian's `Lintian::Spelling`, shared by the Rules
//! that check text for misspellings: `check_spelling` over the vendored
//! `spelling/corrections` and `spelling/corrections-multiword`, and
//! `check_spelling_picky` over `spelling/corrections-case`.
//!
//! A caller passes the exceptions of the lintian group (see
//! [`group_exceptions`]) or none.

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

/// Which `MULTIWORD_LINES` entries are checked for every text: the ones with
/// lookaround (which `regex` cannot compile), and the ones `MULTIWORD_SET`
/// holds, mapped back to their line index.
static MULTIWORD_SPLIT: LazyLock<(RegexSet, Vec<usize>, Vec<usize>)> = LazyLock::new(|| {
    let (fancy, plain): (Vec<usize>, Vec<usize>) =
        (0..MULTIWORD_LINES.len()).partition(|&index| needs_fancy(MULTIWORD_LINES[index].0));
    let set = RegexSet::new(
        plain
            .iter()
            .map(|&index| format!(r"\b(?:{})\b", MULTIWORD_LINES[index].0)),
    )
    .expect("multiword pattern");
    (set, plain, fancy)
});

/// Patterns that `regex` cannot compile (lookaround) need `fancy-regex`.
fn needs_fancy(pattern: &str) -> bool {
    ["(?=", "(?!", "(?<=", "(?<!"]
        .iter()
        .any(|look| pattern.contains(look))
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

/// The `MULTIWORD_LINES` indices that can match `text`, in file order. A line
/// without a regex match has no fancy-regex match either, so only the lines
/// the `RegexSet` reports and the lookaround lines need a fancy-regex run.
fn multiword_candidates(text: &str) -> Vec<usize> {
    let (set, plain, fancy) = &*MULTIWORD_SPLIT;
    let mut candidates: Vec<usize> = set
        .matches(text)
        .into_iter()
        .map(|set_index| plain[set_index])
        .chain(fancy.iter().copied())
        .collect();
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
    let mut duplicates: HashSet<String> = HashSet::new();
    let mut last_word: Option<String> = None;
    let mut quoted = false;

    for raw in normalized.split_whitespace() {
        let ends_with_punct = raw.ends_with(TRAILING_PUNCTUATION);
        let word = raw.trim_end_matches(TRAILING_PUNCTUATION);

        if duplicate_check {
            if word.matches('"').count() % 2 == 1 {
                quoted = !quoted;
            }
            if last_word.as_deref() == Some(word)
                && !quoted
                && duplicates.insert(word.to_string())
                && !ends_with_punct
                && !repeated_in_parens(text, word)
            {
                found.push((format!("{word} {word} (duplicate word)"), word.to_string()));
            }
            last_word = (is_ascii_alpha(word) && !ends_with_punct).then(|| word.to_string());
        }

        let lcword = word.to_lowercase();
        let Some(correction) = CORRECTIONS.get(lcword.as_str()) else {
            continue;
        };
        if is_one_to_five_capitals(word)
            || exceptions.contains(word)
            || exceptions.contains(&lcword)
        {
            continue;
        }
        let correction = adjust_case(word, correction);
        if seen.insert(lcword) {
            found.push((word.to_string(), correction));
        }
    }

    for index in multiword_candidates(&normalized) {
        let Some(regex) = multiword_regex(index) else {
            continue;
        };
        let Some(matched) = regex
            .captures(&normalized)
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
    let no_brackets: String = text.chars().filter(|c| !matches!(c, '[' | ']')).collect();
    let no_parens = strip_parentheses(&no_brackets);
    let joined = HYPHEN_BREAK.replacen(&no_parens, 1, "$1");
    joined.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `s/(\((?!s\))|(?<!\(s)\))//gi`: `(` is dropped unless `s)` follows it, and
/// `)` is dropped unless `(s` precedes it.
fn strip_parentheses(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    for (index, &c) in chars.iter().enumerate() {
        let keep = match c {
            '(' => {
                chars
                    .get(index + 1)
                    .is_some_and(|n| n.eq_ignore_ascii_case(&'s'))
                    && chars.get(index + 2) == Some(&')')
            }
            ')' => {
                index >= 2 && chars[index - 2] == '(' && chars[index - 1].eq_ignore_ascii_case(&'s')
            }
            _ => true,
        };
        if keep {
            out.push(c);
        }
    }
    out
}

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
}
