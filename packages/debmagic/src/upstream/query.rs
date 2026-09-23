use std::path::Path;
use std::str::FromStr;

use anyhow::{Context, bail};
use debmagic_common::debian::version::PackageVersion;
use regex::Regex;

use crate::upstream::mangle::mangle;
use crate::upstream::watch::{SearchMode, WatchSource};

fn substitute(pattern: &str, package: &str) -> String {
    pattern
        .replace("@PACKAGE@", &regex::escape(package))
        .replace(
            "@ANY_VERSION@",
            r"[-_]?[Vv]?(\d[\-+\.:\~\da-zA-Z]*)",
        )
        .replace(
            "@SEMANTIC_VERSION@",
            r"[-_]?[Vv]?((?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)(?:-(?:(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*)(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?(?:\+(?:[0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?)",
        )
        .replace("@STABLE_VERSION@", r"[-_]?[Vv]?((?:[1-9]\d*)(?:\.\d+){2})")
        .replace(
            "@ARCHIVE_EXT@",
            r"(?i)(?:\.(?:tar\.xz|tar\.bz2|tar\.gz|tar\.zstd?|zip|tgz|tbz|txz))",
        )
        .replace(
            "@SIGNATURE_EXT@",
            r"(?i)(?:\.(?:tar\.xz|tar\.bz2|tar\.gz|tar\.zstd?|zip|tgz|tbz|txz))'(?:\.(?:asc|pgp|gpg|sig|sign))'",
        )
        .replace("@DEB_EXT@", r"[\+~](?:debian|dfsg|ds|deb)(?:\.)?(?:\d+)?$")
}

/// Extract the href values from an HTML listing.
pub fn extract_hrefs(html: &str) -> Vec<String> {
    let mut hrefs = Vec::new();
    let mut rest = html;
    while let Some(start) = rest.find("href") {
        rest = &rest[start..];
        let Some(open) = rest.find(['"', '\'']) else {
            break;
        };
        let quote = rest.as_bytes()[open] as char;
        let after_open = &rest[open + 1..];
        let Some(close) = after_open.find(quote) else {
            break;
        };
        hrefs.push(after_open[..close].to_string());
        rest = &after_open[close + 1..];
    }
    hrefs
}

/// A candidate upstream release found by a query.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub version: String,
    pub href: String,
}

/// The archive extensions `@ARCHIVE_EXT@` stands for, in probe order.
const ARCHIVE_EXTS: &[&str] = &[
    "tar.gz", "tar.xz", "tar.bz2", "tar.zst", "zip", "tgz", "tbz", "txz",
];

/// The `[-_]?[Vv]?` prefixes the version macros may carry in filenames.
const VERSION_PREFIXES: &[&str] = &["", "v", "-v", "_v", "V", "-V", "_V", "-", "_"];

/// Turn regex literal escapes (`\.` → `.`) into plain text for template
/// construction.
fn unescape_regex_literals(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\'
            && let Some(next) = chars.next()
            && !next.is_alphanumeric()
        {
            out.push(next);
        } else {
            out.push(c);
        }
    }
    out
}

/// Whether the pattern, outside its macros, is plain literal text —
/// character classes, quantifiers or alternations make the template
/// ambiguous and are not invertible.
fn is_literal_template(template: &str) -> bool {
    !template.contains(['[', ']', '{', '}', '(', ')', '|', '*', '+', '?'])
}

/// Construct the hrefs a concrete `version` would have, directly from
/// the watch pattern — the pattern is a URL template. Returns `None`
/// when the pattern is not invertible: a `Uversion-Mangle` (not
/// invertible), no version macro, or more than one capture group
/// (uscan joins groups with `.`; only one group is unambiguous).
/// Each constructed href must self-check: match the anchored pattern
/// and capture exactly `version`. `preferred_ext` (the extension of
/// the project's existing orig tarball, if any) is ordered first, so
/// the common case probes once.
fn construct_hrefs(
    source: &WatchSource,
    package: &str,
    version: &str,
    preferred_ext: Option<&str>,
) -> anyhow::Result<Option<Vec<String>>> {
    let Some(hrefs) = construct_hrefs_inner(source, package, version, preferred_ext)? else {
        return Ok(None);
    };
    Ok(Some(hrefs))
}

/// [`construct_hrefs`] with a learned example: `learned` is a concrete
/// href from the listing (any version). Its prefix/extension shape
/// is extracted by matching it against the pattern and replacing the
/// captured version back with the requested one — so even patterns
/// pure inversion rejects (multi-group, non-literal) construct once
/// one listed sibling matches.
fn construct_hrefs_from_example(
    source: &WatchSource,
    package: &str,
    version: &str,
    learned: &str,
) -> anyhow::Result<Option<String>> {
    let pattern = substitute(&source.matching_pattern, package);
    let regex = Regex::new(&format!("^{pattern}$"))
        .with_context(|| format!("invalid matching pattern: {pattern}"))?;
    let Some(captures) = regex.captures(learned) else {
        return Ok(None);
    };
    // the learned href's version spans all capture groups (uscan joins
    // them with `.`); replace the whole span with the requested version
    let Some(first) = captures.get(1) else {
        return Ok(None);
    };
    let last = captures
        .get(captures.len() - 1)
        .expect("capture group 1 exists, so does the last");
    let href = format!(
        "{}{version}{}",
        &learned[..first.start()],
        &learned[last.end()..]
    );
    // self-check: the constructed href must match and capture the
    // requested version
    let Some(check) = regex.captures(&href) else {
        return Ok(None);
    };
    let mut joined = String::new();
    for group in 1..check.len() {
        if let Some(part) = check.get(group) {
            if !joined.is_empty() {
                joined.push('.');
            }
            joined.push_str(part.as_str());
        }
    }
    if joined != version {
        return Ok(None);
    }
    Ok(Some(href))
}

fn construct_hrefs_inner(
    source: &WatchSource,
    package: &str,
    version: &str,
    preferred_ext: Option<&str>,
) -> anyhow::Result<Option<Vec<String>>> {
    if source.uversion_mangle.is_some() {
        return Ok(None);
    }
    let raw = &source.matching_pattern;
    if !raw.contains("@ANY_VERSION@")
        && !raw.contains("@SEMANTIC_VERSION@")
        && !raw.contains("@STABLE_VERSION@")
    {
        return Ok(None);
    }
    let pattern = substitute(raw, package);
    let regex = Regex::new(&format!("^{pattern}$"))
        .with_context(|| format!("invalid matching pattern: {pattern}"))?;
    // captures_len() counts the implicit group 0: exactly one capture
    // group means the whole version sits in one place
    if regex.captures_len() != 2 {
        return Ok(None);
    }

    let template = unescape_regex_literals(raw);
    if !is_literal_template(&template) {
        return Ok(None);
    }
    let mut exts: Vec<String> = if raw.contains("@ARCHIVE_EXT@") {
        ARCHIVE_EXTS.iter().map(|e| format!(".{e}")).collect()
    } else {
        vec![String::new()]
    };
    // the project's own orig tarball extension is the strongest hint
    // for what upstream uses; probe it first
    if let Some(preferred) = preferred_ext
        && let Some(index) = exts.iter().position(|e| e == preferred)
    {
        let hint = exts.remove(index);
        exts.insert(0, hint);
    }

    let mut hrefs = Vec::new();
    for ext in &exts {
        for prefix in VERSION_PREFIXES {
            let href = template
                .replace("@PACKAGE@", package)
                .replace("@ANY_VERSION@", &format!("{prefix}{version}"))
                .replace("@SEMANTIC_VERSION@", &format!("{prefix}{version}"))
                .replace("@STABLE_VERSION@", &format!("{prefix}{version}"))
                .replace("@ARCHIVE_EXT@", ext);
            let Some(captures) = regex.captures(&href) else {
                continue;
            };
            if captures.get(1).map(|c| c.as_str()) != Some(version) {
                continue;
            }
            if !hrefs.contains(&href) {
                hrefs.push(href);
            }
        }
    }
    Ok(Some(hrefs))
}

/// Resolve the candidate for a concrete version without scraping the
/// listing: construct the possible hrefs from the watch pattern and
/// probe them. `requested` may be a full debian version — only its
/// upstream part (with `Dversion-Mangle` applied) is used. The
/// extension of an existing orig tarball (output dir, next to the
/// tree, or the cache) is probed first. When no constructed URL
/// exists, the listing is fetched as a *structure teacher*: any
/// listed sibling reveals the prefix/extension shape, and the
/// requested version's URL is constructed from it — which also finds
/// versions that have already fallen out of the listing. Returns
/// `None` when neither works; callers fall back to filtering the
/// listing then.
pub async fn resolve_concrete(
    source: &WatchSource,
    package: &str,
    requested: &str,
    existing_orig: Option<&Path>,
) -> anyhow::Result<Option<Candidate>> {
    // probing goes over http; ftp listings have no HEAD equivalent
    if source.source.starts_with("ftp://") || source.source.starts_with("ftps://") {
        return Ok(None);
    }
    let version = current_upstream_version(source, requested)?;
    let preferred_ext = existing_orig.and_then(|orig| {
        orig.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .and_then(|name| {
                name.split_once(".tar.")
                    .map(|(_, ext)| format!(".tar.{ext}"))
            })
    });
    let hrefs =
        construct_hrefs(source, package, &version, preferred_ext.as_deref())?.unwrap_or_default();

    // probe the constructed hrefs
    for href in &hrefs {
        let candidate = Candidate {
            version: version.clone(),
            href: href.clone(),
        };
        let url = crate::upstream::switch::download_url(source, &candidate)?;
        if crate::requests::http_exists(&url).await {
            return Ok(Some(candidate));
        }
    }

    // nothing constructed exists: let the listing teach the file
    // structure — any listed sibling reveals prefix/extension, and
    // the requested version's URL is constructed from that shape
    let candidates = query_source(source, package).await?;
    if let Some(example) = candidates.first()
        && let Some(href) = construct_hrefs_from_example(source, package, &version, &example.href)?
    {
        let candidate = Candidate {
            version: version.clone(),
            href,
        };
        let url = crate::upstream::switch::download_url(source, &candidate)?;
        if crate::requests::http_exists(&url).await {
            return Ok(Some(candidate));
        }
    }
    Ok(None)
}

/// Query a watch source: fetch its page and return candidate
/// versions, sorted newest first.
pub async fn query_source(source: &WatchSource, package: &str) -> anyhow::Result<Vec<Candidate>> {
    let pattern = substitute(&source.matching_pattern, package);
    // uscan anchors the pattern (^...$): a substring match would also
    // pick up sibling files like foo-1.0.tar.gz.sha256
    let anchored = format!("^{pattern}$");
    let regex =
        Regex::new(&anchored).with_context(|| format!("invalid matching pattern: {pattern}"))?;

    let page = fetch_page(&source.source).await?;
    let is_ftp = source.source.starts_with("ftp://") || source.source.starts_with("ftps://");
    let subjects: Vec<String> = match source.search_mode {
        SearchMode::Html if is_ftp => {
            // ftp listings are `ls -l` style; the filename is the last token
            page.lines()
                .filter_map(|line| line.split_whitespace().next_back())
                .map(str::to_string)
                .collect()
        }
        SearchMode::Html => extract_hrefs(&page),
        SearchMode::Plain => vec![page],
    };

    let mut candidates: Vec<Candidate> = Vec::new();
    for subject in subjects {
        let Some(captures) = regex.captures(&subject) else {
            continue;
        };
        // capture groups join with `.`
        let mut version = String::new();
        for group in 1..captures.len() {
            if let Some(part) = captures.get(group) {
                if !version.is_empty() {
                    version.push('.');
                }
                version.push_str(part.as_str());
            }
        }
        if version.is_empty() {
            continue;
        }
        if let Some(rules) = &source.uversion_mangle {
            version = mangle(rules, &version)?;
        }
        candidates.push(Candidate {
            version,
            href: subject,
        });
    }

    candidates.dedup_by(|a, b| a.version == b.version);

    candidates.sort_by(|a, b| {
        let a = PackageVersion::from_str(&a.version);
        let b = PackageVersion::from_str(&b.version);
        match (a, b) {
            (Ok(a), Ok(b)) => b.cmp(&a),
            // unparseable versions sink to the bottom
            (Ok(_), Err(_)) => std::cmp::Ordering::Less,
            (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
            (Err(_), Err(_)) => std::cmp::Ordering::Equal,
        }
    });
    Ok(candidates)
}

/// Fetch a watch source's page over http(s) or ftp.
async fn fetch_page(url: &str) -> anyhow::Result<String> {
    if url.starts_with("ftp://") || url.starts_with("ftps://") {
        crate::requests::ftp_get(url).await
    } else {
        crate::requests::http_get(url).await
    }
}

/// The changelog's current upstream version, with the source's
/// `dversion_mangle` rules applied for comparison against candidates.
pub fn current_upstream_version(
    source: &WatchSource,
    changelog_version: &str,
) -> anyhow::Result<String> {
    let version = PackageVersion::from_str(changelog_version)
        .map_err(|_| anyhow::anyhow!("invalid changelog version: {changelog_version}"))?;
    let upstream = version.upstream_version().to_string();
    match &source.dversion_mangle {
        Some(rules) => mangle(rules, &upstream),
        None => Ok(upstream),
    }
}

/// Find the candidate for a requested version. The request may be a
/// full debian version like `3.2.23-1` — only its upstream part is
/// used, since candidates carry pure upstream versions.
pub fn find_candidate<'a>(candidates: &'a [Candidate], requested: &str) -> Option<&'a Candidate> {
    let upstream = PackageVersion::from_str(requested)
        .ok()
        .map(|v| v.upstream_version().to_string())
        .unwrap_or_else(|| requested.to_string());
    candidates.iter().find(|c| c.version == upstream)
}

pub fn load_watch(source_dir: &Path) -> anyhow::Result<Vec<WatchSource>> {
    let watch_path = source_dir.join("debian").join("watch");
    if !watch_path.is_file() {
        bail!(
            "{} has no debian/watch file; declare the upstream source in debmagic.toml instead",
            source_dir.display()
        );
    }
    crate::upstream::watch::parse_watch_file(&watch_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_substitute_any_version() {
        let pattern = substitute("foo-@ANY_VERSION@@ARCHIVE_EXT@", "foo");
        let regex = Regex::new(&pattern).unwrap();
        let captures = regex.captures("foo-1.2.3.tar.gz").unwrap();
        assert_eq!(captures.get(1).unwrap().as_str(), "1.2.3");
    }

    #[test]
    fn test_construct_hrefs_simple() {
        let source = WatchSource {
            matching_pattern: "haproxy-@ANY_VERSION@@ARCHIVE_EXT@".to_string(),
            ..Default::default()
        };
        let hrefs = construct_hrefs(&source, "haproxy", "3.2.23", None)
            .unwrap()
            .unwrap();
        // the plain form probes first, prefix variants after it
        assert_eq!(hrefs.first().unwrap(), "haproxy-3.2.23.tar.gz");
        assert!(hrefs.contains(&"haproxy-3.2.23.tar.xz".to_string()));
        // the short forms (tgz/tbz/txz) are alternates of the long ones
        // and probe too — the first existing URL wins
        assert!(hrefs.contains(&"haproxy-3.2.23.tgz".to_string()));
    }

    #[test]
    fn test_construct_hrefs_prefers_existing_ext() {
        let source = WatchSource {
            matching_pattern: "haproxy-@ANY_VERSION@@ARCHIVE_EXT@".to_string(),
            ..Default::default()
        };
        // the project's own orig is xz: that extension probes first
        let hrefs = construct_hrefs(&source, "haproxy", "3.2.23", Some(".tar.xz"))
            .unwrap()
            .unwrap();
        assert_eq!(hrefs.first().unwrap(), "haproxy-3.2.23.tar.xz");
        assert_eq!(hrefs.get(1).unwrap(), "haproxy-v3.2.23.tar.xz");
    }

    #[test]
    fn test_construct_from_example() {
        // a listed sibling teaches the shape: prefix and extension
        let source = WatchSource {
            matching_pattern: "haproxy-@ANY_VERSION@@ARCHIVE_EXT@".to_string(),
            ..Default::default()
        };
        let href =
            construct_hrefs_from_example(&source, "haproxy", "3.2.23", "haproxy-v3.2.24.tar.xz")
                .unwrap()
                .unwrap();
        assert_eq!(href, "haproxy-v3.2.23.tar.xz");

        // a pattern pure inversion rejects (two capture groups) still
        // constructs from an example
        let source = WatchSource {
            matching_pattern: r"foo-v(\d+)\.(\d+)@ARCHIVE_EXT@".to_string(),
            ..Default::default()
        };
        assert!(
            construct_hrefs(&source, "foo", "1.0", None)
                .unwrap()
                .is_none()
        );
        let href = construct_hrefs_from_example(&source, "foo", "2.0", "foo-v1.0.tar.gz")
            .unwrap()
            .unwrap();
        assert_eq!(href, "foo-v2.0.tar.gz");

        // an example that does not match the pattern teaches nothing
        let source = WatchSource {
            matching_pattern: "haproxy-@ANY_VERSION@@ARCHIVE_EXT@".to_string(),
            ..Default::default()
        };
        assert!(
            construct_hrefs_from_example(&source, "haproxy", "3.2.23", "other-1.0.tar.gz")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn test_construct_hrefs_version_prefix() {
        // the [-_]?[Vv]? prefix: v-prefixed releases construct too
        let source = WatchSource {
            matching_pattern: "foo-@ANY_VERSION@@ARCHIVE_EXT@".to_string(),
            ..Default::default()
        };
        let hrefs = construct_hrefs(&source, "foo", "1.0", None)
            .unwrap()
            .unwrap();
        assert!(hrefs.contains(&"foo-v1.0.tar.gz".to_string()));
        assert!(hrefs.contains(&"foo-1.0.tar.gz".to_string()));
    }

    #[test]
    fn test_construct_hrefs_dotted_pattern() {
        // regex literal escapes are unescaped for the template
        let source = WatchSource {
            matching_pattern: r"foo_@ANY_VERSION@\.tar\.gz".to_string(),
            ..Default::default()
        };
        let hrefs = construct_hrefs(&source, "foo", "1.2.3", None)
            .unwrap()
            .unwrap();
        assert!(hrefs.contains(&"foo_1.2.3.tar.gz".to_string()));

        // a character class outside the macros: not invertible
        let source = WatchSource {
            matching_pattern: r"foo[0-9.]+_@ANY_VERSION@\.tar\.gz".to_string(),
            ..Default::default()
        };
        assert!(
            construct_hrefs(&source, "foo", "1.2.3", None)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn test_construct_hrefs_not_invertible() {
        // no version macro
        let source = WatchSource {
            matching_pattern: "foo-latest.tar.gz".to_string(),
            ..Default::default()
        };
        assert!(
            construct_hrefs(&source, "foo", "1.0", None)
                .unwrap()
                .is_none()
        );

        // uversion_mangle: not invertible
        let source = WatchSource {
            matching_pattern: "foo-@ANY_VERSION@@ARCHIVE_EXT@".to_string(),
            uversion_mangle: Some("s/$/+dfsg/".to_string()),
            ..Default::default()
        };
        assert!(
            construct_hrefs(&source, "foo", "1.0", None)
                .unwrap()
                .is_none()
        );

        // more than one capture group: ambiguous how the version splits
        let source = WatchSource {
            matching_pattern: "foo-(\\d+)\\.(\\d+)@ARCHIVE_EXT@".to_string(),
            ..Default::default()
        };
        assert!(
            construct_hrefs(&source, "foo", "1.0", None)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn test_extract_hrefs() {
        let html = r#"<a href="a-1.0.tar.gz">x</a> <a href='b-2.0.tar.gz'>y</a>"#;
        assert_eq!(
            extract_hrefs(html),
            vec!["a-1.0.tar.gz".to_string(), "b-2.0.tar.gz".to_string()]
        );
    }

    #[test]
    fn test_version_concatenation() {
        // multiple capture groups join with `.`
        let pattern = substitute("foo_v(\\d+)_(\\d+)@ARCHIVE_EXT@", "foo");
        let regex = Regex::new(&pattern).unwrap();
        let captures = regex.captures("foo_v1_2.tar.gz").unwrap();
        let mut version = String::new();
        for group in 1..captures.len() {
            if let Some(part) = captures.get(group) {
                if !version.is_empty() {
                    version.push('.');
                }
                version.push_str(part.as_str());
            }
        }
        assert_eq!(version, "1.2");
    }

    #[test]
    fn test_current_upstream_version_dfsg() {
        let source = WatchSource {
            dversion_mangle: Some("s/\\+dfsg\\d*$//".to_string()),
            ..Default::default()
        };
        assert_eq!(
            current_upstream_version(&source, "2.03+dfsg-4").unwrap(),
            "2.03"
        );
    }
    #[test]
    fn test_pattern_anchored_no_sibling_files() {
        // a .sha256 sibling of the tarball must not match the pattern
        let source = WatchSource {
            source: "https://example.com/".to_string(),
            matching_pattern: "foo-@ANY_VERSION@@ARCHIVE_EXT@".to_string(),
            ..Default::default()
        };
        let pattern = substitute(&source.matching_pattern, "foo");
        let regex = Regex::new(&format!("^{pattern}$")).unwrap();
        assert!(regex.captures("foo-1.0.tar.gz").is_some());
        assert!(regex.captures("foo-1.0.tar.gz.sha256").is_none());
        assert!(regex.captures("foo-1.0.tar.gz.asc").is_none());
    }

    #[test]
    fn test_find_candidate_accepts_debian_version() {
        let candidates = vec![
            Candidate {
                version: "3.2.22".to_string(),
                href: "old".to_string(),
            },
            Candidate {
                version: "3.2.23".to_string(),
                href: "new".to_string(),
            },
        ];
        // a full debian version: only the upstream part is used
        let found = find_candidate(&candidates, "3.2.23-1").unwrap();
        assert_eq!(found.version, "3.2.23");
        // a plain upstream version still works
        assert_eq!(
            find_candidate(&candidates, "3.2.22").unwrap().version,
            "3.2.22"
        );
        // an epoch is stripped too
        assert_eq!(
            find_candidate(&candidates, "1:3.2.23-1").unwrap().version,
            "3.2.23"
        );
        // unknown versions find nothing
        assert!(find_candidate(&candidates, "9.9.9").is_none());
    }
}
