use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::debian_changelog::ChangelogEntry;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::installed::{self, InstalledChangelog};

pub struct ChangelogNewsDebianMismatch;

declare_rule! {
    /// `NEWS.Debian` and the installed changelog disagree on distribution or urgency.
    ///
    /// Both files are the ones `debian/changelog` reads for a binary package:
    /// `usr/share/doc/<pkg>/NEWS.Debian.gz` and the installed changelog. Only
    /// the newest entry of each is compared, and only when their versions are
    /// equal and non-empty. Urgency is the first token of the `urgency` value,
    /// or `unknown` when the key is absent. Extra is `Field <changelog> !=
    /// <news>`, distribution first. The Location is the news heading. Udebs
    /// are not checked.
    ChangelogNewsDebianMismatch,
    code = "LN0380",
    tag = "changelog-news-debian-mismatch",
    default_selected = true,
    experimental = false,
}

static KV: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^([-0-9a-z]+)=\s*(.*\S)$").expect("valid regex"));
static URGENCY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^([-0-9a-z]+)((\s+.*)?)$").expect("valid regex"));

impl BinaryPackageRule for ChangelogNewsDebianMismatch {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(changelog) = installed::installed_changelog(ctx) else {
            return;
        };
        let Some(news) = installed::installed_news(ctx) else {
            return;
        };
        let (Some(latest), Some(news_entry)) = (
            changelog.parsed.entries().first(),
            news.parsed.entries().first(),
        ) else {
            return;
        };
        if news_entry.version().is_empty() || news_entry.version() != latest.version() {
            return;
        }
        let mut messages = Vec::new();
        for field in ["Distribution", "Urgency"] {
            let from = field_value(&changelog, latest, field);
            let to = field_value(&news, news_entry, field);
            if from != to {
                messages.push(format!("{field} {from} != {to}"));
            }
        }
        let path = news.path.clone();
        let line = news_entry.line();
        for message in messages {
            ctx.diagnostic(Diagnostic::warn(message).with_location(Location {
                path: path.clone(),
                line: Some(line),
                column: None,
            }));
        }
    }
}

fn field_value(installed: &InstalledChangelog, entry: &ChangelogEntry, field: &str) -> String {
    if field == "Distribution" {
        return entry.distribution().to_string();
    }
    urgency(installed.text.as_str(), entry.line())
}

fn urgency(text: &str, line: u32) -> String {
    let Some(heading) = text.lines().nth(line.saturating_sub(1) as usize) else {
        return "unknown".to_string();
    };
    let Some((_, kvpairs)) = heading.split_once(';') else {
        return "unknown".to_string();
    };
    for raw in kvpairs.split(',') {
        let pair = raw.trim();
        let Some(captures) = KV.captures(pair) else {
            continue;
        };
        let key_raw = captures.get(1).map(|m| m.as_str()).unwrap_or("");
        let mut key_chars = key_raw.chars();
        let key = match key_chars.next() {
            Some(first) => {
                let mut s = first.to_uppercase().collect::<String>();
                s.push_str(key_chars.as_str());
                s
            }
            None => String::new(),
        };
        if key != "Urgency" {
            continue;
        }
        let value = captures.get(2).map(|m| m.as_str()).unwrap_or("");
        let Some(token) = URGENCY.captures(value).and_then(|c| c.get(1)) else {
            continue;
        };
        return token.as_str().to_string();
    }
    "unknown".to_string()
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn entry(version: &str, distribution: &str, urgency: &str) -> String {
        format!(
            "example ({version}) {distribution}; urgency={urgency}\n\n  * A change.\n\n -- A Uthor <a@example.com>  Tue, 30 Dec 2008 17:34:02 +0000\n"
        )
    }

    fn gzip(text: &str) -> TestFile {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(text.as_bytes()).expect("gzip");
        TestFile::from_bytes(encoder.finish().expect("gzip"))
    }

    fn packaged(changelog: &str, news: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![
            (
                "usr/share/doc/example/changelog.gz".to_string(),
                gzip(changelog),
            ),
            (
                "usr/share/doc/example/NEWS.Debian.gz".to_string(),
                gzip(news),
            ),
        ])
    }

    #[test]
    fn test() {
        let same = entry("1.0-1", "unstable", "low");
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            packaged(&same, &same),
            packaged(
                &entry("1.0-1", "unstable", "low"),
                &entry("2.0-1", "UNRELEASED", "high"),
            ),
            packaged(
                &entry("1.0-1", "unstable", "low"),
                &entry("1.0-1", "UNRELEASED", "high"),
            )
            .udeb(),
        ];
        let fail = vec![packaged(
            &entry("1.0-1", "unstable", "low"),
            &entry("1.0-1", "UNRELEASED", "high"),
        )];
        BinaryPackageTester::new(ChangelogNewsDebianMismatch, pass, fail).test_and_snapshot();
    }
}
