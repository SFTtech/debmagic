use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::data::lines;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::license_text::{self, TextFile};
use crate::lint::rule::SourceRule;

pub struct LicenseProblemGfdlInvariants;

declare_rule! {
    /// A Patched file is under the GNU Free Documentation License with
    /// invariant sections, front-cover texts or back-cover texts, which are
    /// not suitable for main. The hint carries the invariant part as
    /// `invariant part is: <text>`, with punctuation and line numbers stripped.
    ///
    /// The file's cleaned text is searched for the license sentence and its
    /// invariant part as lintian's `cruft` Check does. The invariant part is
    /// then compared with the known-good wordings of `cruft/gfdl-license-fragments-checks`
    /// (the official "no invariant sections" wording, the examples of the
    /// license notice, and the fragments accepted for some file names).
    /// Skipped in a non-free Source package (see `license_text::is_non_free`),
    /// when the invariant part is empty, and for paths under `.pc/`. Each
    /// finding has the path as the Location.
    ///
    /// Differs from lintian: the file is matched whole, not in 16 KiB
    /// sliding-window blocks. The fragment tables of the other GFDL Tags
    /// (`license-problem-gfdl-non-official-text` and the empty-invariant
    /// variant) are not reported here.
    LicenseProblemGfdlInvariants,
    code = "LN1302",
    tag = "license-problem-gfdl-invariants",
    default_selected = true,
    experimental = false,
}

const KEYWORDS: &[&str] = &["license", "document", "gnu", "copy", "documentation"];
const SENTENCE: &str = "gnu free documentation license";
/// Window of lintian's `(?!...)` loops, in characters.
const WINDOW: usize = 1024;
const NO_INVARIANT_PREFIX: &str = "a copy of the license is";
const DISTRIBUTED: &str = "this document is distributed";

/// Lintian's Perl pattern under `/msx` (or `/xis`, given by `flags`): `[ ]`
/// is a space, unescaped whitespace is ignored, `\Z` is the end.
fn perl(flags: &str, pattern: &str) -> Regex {
    let pattern = pattern.replace("[ ]", r"\x20").replace(r"\Z", r"\z");
    Regex::new(&format!("(?{flags}){pattern}")).expect("valid regex")
}

/// The text of the lintian `cruft` GFDL match: the characters before the
/// license name (`rawcontextbefore`) and the invariant part
/// (`rawgfdlsections`).
struct GfdlCapture {
    context: String,
    sections: String,
}

fn starts_with_at(text: &[char], at: usize, pattern: &str) -> bool {
    pattern
        .chars()
        .enumerate()
        .all(|(offset, c)| text.get(at + offset) == Some(&c))
}

/// Context alternatives of lintian's GFDL regex for the license name at
/// `p`, leftmost first: exactly 1024 characters before `p`, at most 1024
/// characters from the start of the text, or " copy of the license is"
/// followed by at most 1024 characters. Returns the start of the context.
fn context_start(text: &[char], p: usize) -> Option<usize> {
    let none_starts =
        |from: usize| (from..p).all(|i| !starts_with_at(text, i, NO_INVARIANT_PREFIX));
    let mut starts = Vec::new();
    if p >= WINDOW && none_starts(p - WINDOW) {
        starts.push(p - WINDOW);
    }
    if p <= WINDOW && none_starts(0) {
        starts.push(0);
    }
    let copy = " copy of the license is";
    let copy_len = copy.chars().count();
    if let Some(s) = (0..p).find(|&s| {
        starts_with_at(text, s, copy) && s + copy_len <= p && p - (s + copy_len) <= WINDOW
    }) {
        starts.push(s);
    }
    starts.into_iter().min()
}

/// The lazy invariant part from `from`: the shortest run of at most 1024
/// characters, none starting the license name, that is followed by one of the
/// terminators. Returns the end of the run.
fn sections_end(text: &[char], from: usize) -> Option<usize> {
    let mut i = from;
    loop {
        if starts_with_at(text, i, NO_INVARIANT_PREFIX) || starts_with_at(text, i, DISTRIBUTED) {
            return Some(i);
        }
        if i - from == WINDOW || i >= text.len() || starts_with_at(text, i, SENTENCE) {
            return None;
        }
        i += 1;
    }
}

/// The first place the license name is followed by an invariant part and a
/// terminator (lintian's `$gfdl_regex`), with its context before it.
fn find_gfdl(clean: &str) -> Option<GfdlCapture> {
    let text: Vec<char> = clean.chars().collect();
    let name_len = SENTENCE.chars().count();
    (0..text.len()).find_map(|p| {
        if !starts_with_at(&text, p, SENTENCE) {
            return None;
        }
        let context_from = context_start(&text, p)?;
        let sections_from = p + name_len;
        let sections_to = sections_end(&text, sections_from)?;
        Some(GfdlCapture {
            context: text[context_from..p].iter().collect(),
            sections: text[sections_from..sections_to].iter().collect(),
        })
    })
}

static STRIP_PUNCT: LazyLock<Regex> =
    LazyLock::new(|| perl("msx", r"\s*[,\.;]\s*\Z|\A\s*[,\.;]\s*"));

/// lintian's `_strip_punct`: final and leading punctuation becomes a space,
/// whitespace is collapsed and the text trimmed.
fn strip_punct(text: &str) -> String {
    let text = STRIP_PUNCT.replace_all(text, " ");
    let collapsed = Regex::new(r"\s+")
        .expect("valid regex")
        .replace_all(&text, " ");
    collapsed.trim().to_string()
}

/// lintian's line-number removal: ` 12 `, a leading `12 ` and a trailing
/// ` 12` (the first occurrence only) become spaces, then runs of spaces collapse.
static LINE_NUMBER_INNER: LazyLock<Regex> = LazyLock::new(|| perl("msx", r"[ ]\d+[ ]"));
static LINE_NUMBER_START: LazyLock<Regex> = LazyLock::new(|| perl("msx", r"^\d+[ ]"));
static LINE_NUMBER_END: LazyLock<Regex> = LazyLock::new(|| perl("msx", r"[ ]\d+$"));
static SPACE_RUNS: LazyLock<Regex> = LazyLock::new(|| perl("msx", r"[ ]+"));

/// The classical wording lintian removes from the start and end of the
/// invariant part, applied until nothing changes.
static CLASSICAL: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"\A \(?[ ]? g?fdl [ ]?\)?[ ]? [,\.;]?[ ]?",
        r"\A (?:either[ ])? version [ ] \d+(?:\.\d+)? [ ]?",
        r"\A of [ ] the [ ] license [ ]?[,\.;][ ]?",
        r"\A or (?:[ ]\(?[ ]? at [ ] your [ ] option [ ]?\)?)? [ ] any [ ] later [ ] version[ ]?",
        r"\A (as[ ])? published [ ] by [ ] the [ ] free [ ] software [ ] foundation[ ]?",
        r"\(?[ ]? fsf [ ]?\)?[ ]?",
        r"\A [ ]? [,\.;]? [ ]?",
        r"[ ]? [,\.]? [ ]?\Z",
    ]
    .iter()
    .map(|pattern| perl("msx", pattern))
    .collect()
});

static PERMISSION_TAIL: LazyLock<Regex> = LazyLock::new(|| {
    perl(
        "msx",
        r"[ ]? (:?[,\.;]? [ ]?)?
        permission [ ] is [ ] granted [ ] to [ ] copy [ ]?[,\.;]?[ ]?
        distribute [ ]?[,\.;]?[ ]? and[ ]?/?[ ]?or [ ] modify [ ]
        this [ ] document [ ] under [ ] the [ ] terms [ ] of [ ] the\Z",
    )
});

static OFFICIAL: LazyLock<Regex> = LazyLock::new(|| {
    perl(
        "msx",
        r"\A
        with [ ] no [ ] invariant [ ] sections[ ]?,
        [ ]? no [ ] front(?:[ ]?-[ ]?|[ ])cover [ ] texts[ ]?,?
        [ ]? and [ ] no [ ] back(?:[ ]?-?[ ]?|[ ])cover [ ] texts
        \Z",
    )
});

static EXAMPLE: LazyLock<Regex> = LazyLock::new(|| {
    perl(
        "msx",
        r"following [ ] is [ ] an [ ] example
        (:?[ ] of [ ] the [ ] license [ ] notice [ ] to [ ] use
        (?:[ ] after [ ] the [ ] copyright [ ] (?:line(?:\(s\)|s)?)?
        (?:[ ] using [ ] all [ ] the [ ] features? [ ] of [ ] the [ ] gfdl)?
        )?
        )? [ ]? [,:]? \Z",
    )
});

/// One line of `cruft/gfdl-license-fragments-checks`: a known-good invariant
/// part, accepted only for paths matching `accept`.
struct Fragment {
    section: Regex,
    accept: Regex,
}

/// The data table: `<section regex> [~~ <accepted paths> [~~ <tag>]]`.
/// Only the section and the accepted paths decide here; the tags belong to
/// the other GFDL Rules.
static FRAGMENTS: LazyLock<Vec<Fragment>> = LazyLock::new(|| {
    lines(include_str!(
        "../../../data/cruft/gfdl-license-fragments-checks"
    ))
    .map(|line| {
        let (section, rest) = line.split_once("~~").unwrap_or((line, ""));
        let (accept, _tag) = match rest.split_once("~~") {
            Some((accept, tag)) => (accept, tag),
            None => (rest, ""),
        };
        let accept = accept.trim();
        let accept = if accept.is_empty() { ".*" } else { accept };
        Fragment {
            section: perl("isx", section.trim()),
            accept: perl("sx", accept),
        }
    })
    .collect()
});

/// lintian's `check_gfdl_license_problem` for the invariant part and context
/// of one file: the `invariant part is:` extra when the file is flagged.
fn gfdl_problem(file: &TextFile) -> Option<String> {
    let capture = find_gfdl(&file.clean)?;
    let mut sections = strip_punct(&capture.sections);
    let mut context = strip_punct(&capture.context);

    sections = LINE_NUMBER_INNER.replace_all(&sections, " ").into_owned();
    sections = LINE_NUMBER_START.replace(&sections, " ").into_owned();
    sections = LINE_NUMBER_END.replace(&sections, " ").into_owned();
    sections = SPACE_RUNS.replace_all(&sections, " ").into_owned();

    let mut changed = true;
    while changed {
        let before = sections.clone();
        for pattern in CLASSICAL.iter() {
            sections = pattern.replace(&sections, "").into_owned();
        }
        changed = sections != before;
    }

    context = PERMISSION_TAIL.replace(&context, "").into_owned();

    if sections.is_empty() || OFFICIAL.is_match(&sections) || EXAMPLE.is_match(&context) {
        return None;
    }

    let path = file.path.to_string_lossy();
    for fragment in FRAGMENTS.iter() {
        if fragment.section.is_match(&sections) {
            return if fragment.accept.is_match(&path) {
                None
            } else {
                Some(format!("invariant part is: {sections}"))
            };
        }
    }
    Some(format!("invariant part is: {sections}"))
}

impl SourceRule for LicenseProblemGfdlInvariants {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        if license_text::is_non_free(ctx) {
            return;
        }
        let Some(files) = ctx.files() else {
            return;
        };
        let flagged: Vec<(PathBuf, String)> = license_text::text_files(files)
            .into_iter()
            .filter(|file| {
                KEYWORDS
                    .iter()
                    .any(|keyword| file.lowercase.contains(keyword))
                    && file.clean.contains(SENTENCE)
            })
            .filter_map(|file| gfdl_problem(&file).map(|extra| (file.path, extra)))
            .collect();
        for (path, extra) in flagged {
            ctx.diagnostic(Diagnostic::error(extra).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestCase};

    use super::*;

    const OFFICIAL_NOTICE: &str = "Permission is granted to copy, distribute and/or modify this document under the terms of the GNU Free Documentation License, Version 1.3 with no Invariant Sections, no Front-Cover Texts, and no Back-Cover Texts. A copy of the license is included in the section entitled \"GNU Free Documentation License\".\n";

    const INVARIANT_NOTICE: &str = "Permission is granted to copy, distribute and/or modify this document under the terms of the GNU Free Documentation License, Version 1.2 with the Invariant Sections being GNU Manifesto, with the Front-Cover Texts being (A) and with the Back-Cover Texts being (B). A copy of the license is included.\n";

    #[test]
    fn source() {
        let pass = vec![
            TestCase::from(vec![("src/main.c", "/* plain source */\n")]),
            TestCase::from(vec![("doc/COPYING", OFFICIAL_NOTICE)]),
        ];
        let fail = vec![TestCase::from(vec![("doc/manual.txt", INVARIANT_NOTICE)])];
        SourceTreeTester::new(LicenseProblemGfdlInvariants, pass, fail).test_and_snapshot();
    }
}
