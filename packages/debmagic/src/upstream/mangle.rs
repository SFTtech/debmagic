use anyhow::{Context, bail};

/// A `s/regex/replacement/flags` or `tr/src/dest/` (alias `y`) rule,
/// as used by the `*-Mangle` watch file options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MangleRule {
    Subst {
        pattern: String,
        replacement: String,
        global: bool,
        case_insensitive: bool,
    },
    Transliterate {
        from: String,
        to: String,
    },
}

/// Parse a mangle rule string: `;`-separated rules of
/// `s/pattern/replacement/[gi]` or `tr/from/to/` (`y/from/to/`),
/// with any non-alphanumeric delimiter.
pub fn parse_mangle_rules(rules: &str) -> anyhow::Result<Vec<MangleRule>> {
    let mut parsed = Vec::new();
    for rule in rules.split(';') {
        let rule = rule.trim();
        if rule.is_empty() {
            continue;
        }
        let mut chars = rule.chars();
        let op = chars.next().context("empty mangle rule")?;
        match op {
            's' => {
                let delimiter = chars.next().context("missing delimiter in s-rule")?;
                if delimiter.is_alphanumeric() {
                    bail!("invalid delimiter {delimiter:?} in mangle rule: {rule}");
                }
                let (pattern, rest) = split_on_delimiter(chars.as_str(), delimiter)
                    .with_context(|| format!("unterminated pattern in mangle rule: {rule}"))?;
                let (replacement, flags) = split_on_delimiter(&rest, delimiter)
                    .with_context(|| format!("unterminated replacement in mangle rule: {rule}"))?;
                let mut global = false;
                let mut case_insensitive = false;
                for flag in flags.trim().chars() {
                    match flag {
                        'g' => global = true,
                        'i' => case_insensitive = true,
                        _ => bail!("unsupported flag {flag:?} in mangle rule: {rule}"),
                    }
                }
                if pattern.is_empty() {
                    bail!("empty pattern in mangle rule: {rule}");
                }
                parsed.push(MangleRule::Subst {
                    pattern,
                    replacement,
                    global,
                    case_insensitive,
                });
            }
            't' => {
                if chars.next() != Some('r') {
                    bail!("unknown mangle operation in rule: {rule}");
                }
                parsed.push(parse_transliterate(chars.as_str(), rule)?);
            }
            'y' => {
                parsed.push(parse_transliterate(chars.as_str(), rule)?);
            }
            _ => bail!("unsupported mangle operation {op:?} in rule: {rule} (only s, tr, y)"),
        }
    }
    if parsed.is_empty() {
        bail!("no rules found in mangle string: {rules}");
    }
    Ok(parsed)
}

/// Split `s` at the next unescaped `delimiter`, returning (head, tail-after-delimiter).
/// Backslash-escaped delimiters are unescaped in the head.
fn split_on_delimiter(s: &str, delimiter: char) -> Option<(String, String)> {
    let mut head = String::new();
    let mut chars = s.chars();
    loop {
        let c = chars.next()?;
        if c == '\\' {
            let escaped = chars.next()?;
            if escaped == delimiter {
                head.push(delimiter);
            } else {
                head.push('\\');
                head.push(escaped);
            }
        } else if c == delimiter {
            return Some((head, chars.as_str().to_string()));
        } else {
            head.push(c);
        }
    }
}

fn parse_transliterate(s: &str, rule: &str) -> anyhow::Result<MangleRule> {
    let mut chars = s.chars();
    let delimiter = chars
        .next()
        .with_context(|| format!("missing delimiter in tr-rule: {rule}"))?;
    if delimiter.is_alphanumeric() {
        bail!("invalid delimiter {delimiter:?} in mangle rule: {rule}");
    }
    let (from, rest) = split_on_delimiter(chars.as_str(), delimiter)
        .with_context(|| format!("unterminated source in mangle rule: {rule}"))?;
    let (to, _flags) = split_on_delimiter(&rest, delimiter)
        .with_context(|| format!("unterminated destination in mangle rule: {rule}"))?;
    Ok(MangleRule::Transliterate { from, to })
}

/// Apply parsed rules to `input` in order.
pub fn apply_mangle_rules(rules: &[MangleRule], input: &str) -> anyhow::Result<String> {
    let mut result = input.to_string();
    for rule in rules {
        result = match rule {
            MangleRule::Subst {
                pattern,
                replacement,
                global,
                case_insensitive,
            } => {
                let mut builder = regex::RegexBuilder::new(pattern);
                builder.case_insensitive(*case_insensitive);
                let regex = builder
                    .build()
                    .with_context(|| format!("invalid regex in mangle rule: {pattern}"))?;
                if *global {
                    regex.replace_all(&result, replacement).into_owned()
                } else {
                    regex.replace(&result, replacement).into_owned()
                }
            }
            MangleRule::Transliterate { from, to } => {
                let from_chars: Vec<char> = from.chars().collect();
                let to_chars: Vec<char> = to.chars().collect();
                if from_chars.len() < to_chars.len() {
                    bail!("tr source longer than destination: {from} -> {to}");
                }
                // Perl tr semantics: a shorter destination repeats its last char
                result
                    .chars()
                    .map(|c| match from_chars.iter().position(|&f| f == c) {
                        Some(i) => {
                            if i < to_chars.len() {
                                to_chars[i]
                            } else {
                                *to_chars.last().unwrap_or(&c)
                            }
                        }
                        None => c,
                    })
                    .collect()
            }
        };
    }
    Ok(result)
}

/// Parse and apply in one step, the common case for watch file options.
pub fn mangle(rules: &str, input: &str) -> anyhow::Result<String> {
    let parsed = parse_mangle_rules(rules)?;
    apply_mangle_rules(&parsed, input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case("s/foo/bar/", "foo foo", "bar foo"; "single substitution")]
    #[test_case("s/foo/bar/g", "foo foo", "bar bar"; "global substitution")]
    #[test_case("s/FOO/bar/i", "foo", "bar"; "case insensitive")]
    #[test_case("s/a\\/b/X/", "a/b", "X"; "escaped delimiter")]
    #[test_case("s/\\+dfsg\\d*$//", "2.03+dfsg", "2.03"; "dfsg strip")]
    #[test_case(
        "s/(\\d)[_\\.\\-\\+]?((?:RC|rc|pre|dev|beta|alpha)\\d*)$/$1~$2/",
        "1.2.3rc1",
        "1.2.3~rc1"
        ; "uversion auto"
    )]
    #[test_case("s/a/X/;s/b/Y/", "ab", "XY"; "multiple rules")]
    #[test_case("tr/abc/xyz/", "abc", "xyz"; "transliterate")]
    #[test_case("y/abc/xyz/", "abc", "xyz"; "y alias")]
    #[test_case("s%prdownload%download%", "prdownload", "download"; "percent delimiter")]
    fn test_mangle(rules: &str, input: &str, expected: &str) {
        assert_eq!(mangle(rules, input).unwrap(), expected);
    }

    #[test_case("s/foo"; "unterminated")]
    #[test_case("x/foo/bar/"; "unknown op")]
    #[test_case("s//bar/"; "empty pattern")]
    #[test_case("s/foo/bar/z"; "bad flag")]
    fn test_mangle_rejects(rules: &str) {
        assert!(mangle(rules, "foo").is_err());
    }

    #[test]
    fn test_tr_last_char_repeated() {
        // Perl tr semantics: a shorter destination repeats its last char
        assert_eq!(mangle("tr/abc/x/", "cab").unwrap(), "xxx");
    }
}
