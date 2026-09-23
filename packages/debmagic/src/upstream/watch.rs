use std::path::Path;

use anyhow::{Context, bail};

/// A parsed `debian/watch` source: one paragraph (v5) or line (v2–v4).
#[derive(Debug, Clone, Default)]
pub struct WatchSource {
    pub source: String,
    pub matching_pattern: String,
    pub search_mode: SearchMode,
    pub uversion_mangle: Option<String>,
    pub dversion_mangle: Option<String>,
    pub filename_mangle: Option<String>,
    pub download_url_mangle: Option<String>,
    pub pgp_sig_url_mangle: Option<String>,
    pub pgp_mode: Option<PgpMode>,
    pub repack_suffix: Option<String>,
    pub repack: bool,
    /// MUT component name: this source describes one component tarball
    /// of a multiple-upstream-tarballs package.
    pub component: Option<String>,
    pub untrackable: Option<String>,
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub enum SearchMode {
    #[default]
    Html,
    Plain,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PgpMode {
    Auto,
    Default,
    Mangle,
    None,
}

/// Parse `debian/watch` (versions 2–5). Version 1 is rejected;
/// unsupported options produce a clear error.
pub fn parse_watch_file(path: &Path) -> anyhow::Result<Vec<WatchSource>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    parse_watch(&content)
}

/// Parse a watch file's contents. The first non-comment line determines
/// the version: `version=2|3|4` (single-line format) or `Version: 5`
/// (deb822 paragraphs).
pub fn parse_watch(content: &str) -> anyhow::Result<Vec<WatchSource>> {
    let version = detect_version(content)?;
    match version {
        2..=4 => parse_watch_v4(content),
        5 => parse_watch_v5(content),
        _ => bail!("unsupported watch file version {version}"),
    }
}

/// The version declared by the file, from `version=N` or `Version: N`.
fn detect_version(content: &str) -> anyhow::Result<u32> {
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("version=") {
            return rest
                .trim()
                .parse()
                .with_context(|| format!("invalid watch version: {rest}"));
        }
        if let Some(rest) = line.strip_prefix("Version:") {
            return rest
                .trim()
                .parse()
                .with_context(|| format!("invalid watch version: {rest}"));
        }
        bail!(
            "watch file has no version declaration; add 'Version: 5' (version 1 files are not supported)"
        );
    }
    bail!("watch file is empty");
}

/// v2–v4: one `opts=...` line per source, with backslash continuations joined.
fn parse_watch_v4(content: &str) -> anyhow::Result<Vec<WatchSource>> {
    let mut sources = Vec::new();
    let mut logical = String::new();
    for line in content.lines() {
        let line = line.trim_end();
        if let Some(cont) = line.strip_suffix('\\') {
            logical.push_str(cont.trim_end());
            logical.push(' ');
            continue;
        }
        logical.push_str(line);
        let joined = logical.trim().to_string();
        logical.clear();
        if joined.is_empty() || joined.starts_with('#') || joined.starts_with("version=") {
            continue;
        }
        sources.push(parse_watch_line(&joined)?);
    }
    if sources.is_empty() {
        bail!("watch file declares no sources");
    }
    Ok(sources)
}

/// One v2–v4 line: `[opts=...] <url> [matching-pattern]`. The pattern may
/// be a separate token or embedded in the URL as its last `/`-separated
/// component (uscan's shorthand).
fn parse_watch_line(line: &str) -> anyhow::Result<WatchSource> {
    let mut source = WatchSource::default();
    let mut rest = line;
    if let Some(opts) = line.strip_prefix("opts=") {
        let (opts, remainder) = opts
            .split_once(' ')
            .context("opts= without url in watch line")?;
        apply_opts(&mut source, opts)?;
        rest = remainder.trim_start();
    }
    match rest.split_once(' ') {
        Some((url, pattern)) => {
            source.source = url.to_string();
            source.matching_pattern = pattern.to_string();
        }
        // shorthand: the pattern is the URL's last `/` component
        None => {
            let (url, pattern) = rest
                .rsplit_once('/')
                .with_context(|| format!("watch line has no matching pattern: {line}"))?;
            source.source = format!("{url}/");
            source.matching_pattern = pattern.to_string();
        }
    }
    Ok(source)
}

/// v5: deb822 paragraphs; the first paragraph's options are defaults
/// for the following source paragraphs.
fn parse_watch_v5(content: &str) -> anyhow::Result<Vec<WatchSource>> {
    let mut paragraphs: Vec<Vec<(String, String)>> = Vec::new();
    let mut current: Vec<(String, String)> = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !current.is_empty() {
                paragraphs.push(std::mem::take(&mut current));
            }
            continue;
        }
        if trimmed.starts_with('#') {
            continue;
        }
        let (key, value) = trimmed
            .split_once(':')
            .with_context(|| format!("invalid line in watch file: {line}"))?;
        current.push((key.trim().to_lowercase(), value.trim().to_string()));
    }
    if !current.is_empty() {
        paragraphs.push(current);
    }

    let mut sources = Vec::new();
    // first paragraph holds defaults (and may itself be a source)
    let mut defaults = WatchSource::default();
    let mut defaults_are_source = false;
    for (index, paragraph) in paragraphs.iter().enumerate() {
        let mut source = WatchSource::default();
        for (key, value) in paragraph {
            match key.as_str() {
                "version" => {}
                "untrackable" => source.untrackable = Some(value.clone()),
                "source" => {
                    source.source = value.clone();
                    defaults_are_source = index == 0;
                }
                "matching-pattern" | "matchingpattern" => source.matching_pattern = value.clone(),
                "search-mode" | "searchmode" => {
                    source.search_mode = match value.to_lowercase().as_str() {
                        "html" => SearchMode::Html,
                        "plain" => SearchMode::Plain,
                        _ => bail!("invalid Search-Mode: {value}"),
                    }
                }
                "uversion-mangle" | "uversionmangle" => {
                    source.uversion_mangle = Some(value.clone())
                }
                "dversion-mangle" | "dversionmangle" => {
                    source.dversion_mangle = Some(value.clone())
                }
                "filename-mangle" | "filenamemangle" => {
                    source.filename_mangle = Some(value.clone())
                }
                "download-url-mangle" | "downloadurlmangle" => {
                    source.download_url_mangle = Some(value.clone())
                }
                "pgp-sig-url-mangle" | "pgpsigurlmangle" => {
                    source.pgp_sig_url_mangle = Some(value.clone())
                }
                "pgp-mode" | "pgpmode" => {
                    source.pgp_mode = Some(match value.to_lowercase().as_str() {
                        "auto" => PgpMode::Auto,
                        "default" => PgpMode::Default,
                        "mangle" => PgpMode::Mangle,
                        "none" => PgpMode::None,
                        other => bail!("unsupported Pgp-Mode: {other}"),
                    })
                }
                "repack" => source.repack = value.eq_ignore_ascii_case("yes"),
                "repacksuffix" => source.repack_suffix = Some(value.clone()),
                "component" => source.component = Some(value.clone()),
                "ctype" | "version-schema" | "versionseparator" | "update-script"
                | "page-mangle" | "mode" | "compression" | "unzip-opt" | "git-pretty"
                | "git-date" | "git-export" | "git-mode" | "git-modules" | "bare" | "maturity"
                | "hrefdecode" | "decompress" | "user-agent" | "template" | "dist" | "owner"
                | "project" | "oversion-mangle" => {
                    bail!(
                        "watch file option '{key}' is not supported yet; \
                         declare the upstream source in debmagic.toml instead"
                    )
                }
                _ => bail!("unknown watch file option: {key}"),
            }
        }
        if index == 0 {
            // first paragraph: defaults, possibly also a source itself
            defaults = source.clone();
            if defaults_are_source {
                sources.push(source);
            }
        } else {
            // merge over the first paragraph's defaults
            let merged = merge_source(&defaults, source);
            sources.push(merged);
        }
    }
    if sources.is_empty() {
        bail!("watch file declares no sources");
    }
    Ok(sources)
}

/// Field-by-field merge of a source over defaults, `None`/empty fields
/// falling back to the defaults.
fn merge_source(defaults: &WatchSource, source: WatchSource) -> WatchSource {
    let mut merged = source;
    if merged.source.is_empty() {
        merged.source = defaults.source.clone();
    }
    if merged.matching_pattern.is_empty() {
        merged.matching_pattern = defaults.matching_pattern.clone();
    }
    if merged.search_mode == defaults.search_mode {
        // nothing to do; both default or both overridden identically
    }
    merged.uversion_mangle = merged
        .uversion_mangle
        .or_else(|| defaults.uversion_mangle.clone());
    merged.dversion_mangle = merged
        .dversion_mangle
        .or_else(|| defaults.dversion_mangle.clone());
    merged.filename_mangle = merged
        .filename_mangle
        .or_else(|| defaults.filename_mangle.clone());
    merged.download_url_mangle = merged
        .download_url_mangle
        .or_else(|| defaults.download_url_mangle.clone());
    merged.pgp_sig_url_mangle = merged
        .pgp_sig_url_mangle
        .or_else(|| defaults.pgp_sig_url_mangle.clone());
    merged.pgp_mode = merged.pgp_mode.or(defaults.pgp_mode);
    merged.repack_suffix = merged
        .repack_suffix
        .or_else(|| defaults.repack_suffix.clone());
    merged.repack = merged.repack || defaults.repack;
    merged.component = merged.component.or_else(|| defaults.component.clone());
    merged
}

/// Apply `opts=` content of a v2–v4 line.
fn apply_opts(source: &mut WatchSource, opts: &str) -> anyhow::Result<()> {
    for opt in opts.split(',') {
        let opt = opt.trim();
        if let Some(value) = opt.strip_prefix("uversionmangle=") {
            source.uversion_mangle = Some(value.to_string());
        } else if let Some(value) = opt.strip_prefix("dversionmangle=") {
            source.dversion_mangle = Some(value.to_string());
        } else if let Some(value) = opt.strip_prefix("filenamemangle=") {
            source.filename_mangle = Some(value.to_string());
        } else if let Some(value) = opt.strip_prefix("downloadurlmangle=") {
            source.download_url_mangle = Some(value.to_string());
        } else if let Some(value) = opt.strip_prefix("pgpsigurlmangle=") {
            source.pgp_sig_url_mangle = Some(value.to_string());
        } else if let Some(value) = opt.strip_prefix("pgpmode=") {
            source.pgp_mode = Some(match value {
                "auto" => PgpMode::Auto,
                "default" => PgpMode::Default,
                "mangle" => PgpMode::Mangle,
                "none" => PgpMode::None,
                other => bail!("unsupported pgpmode: {other}"),
            });
        } else if let Some(value) = opt.strip_prefix("repacksuffix=") {
            source.repack_suffix = Some(value.to_string());
        } else if opt == "repack" {
            source.repack = true;
        } else if opt == "pasv" || opt == "passive" {
            // FTP-only option, irrelevant for our fetcher
        } else if let Some(value) = opt.strip_prefix("searchmode=") {
            source.search_mode = match value {
                "html" => SearchMode::Html,
                "plain" => SearchMode::Plain,
                _ => bail!("invalid searchmode: {value}"),
            };
        } else {
            bail!(
                "watch file option '{opt}' is not supported yet; \
                 declare the upstream source in debmagic.toml instead"
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test]
    fn test_parse_v4_basic() {
        let sources =
            parse_watch("version=4\nhttps://example.com/releases/ foo-(.*)\\.tar\\.gz\n").unwrap();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].source, "https://example.com/releases/");
        assert_eq!(sources[0].matching_pattern, "foo-(.*)\\.tar\\.gz");
    }

    #[test]
    fn test_parse_v4_opts() {
        let sources = parse_watch(
            "version=4\n\
             opts=uversionmangle=s/(\\d)[_\\.\\-\\+]?((?:RC|rc|pre|dev|beta|alpha)\\d*)$/$1~$2/ \\\n\
             https://example.com/ foo-(.*)\\.tar\\.gz\n",
        )
        .unwrap();
        assert!(sources[0].uversion_mangle.is_some());
    }

    #[test]
    fn test_parse_v5_basic() {
        let sources = parse_watch(
            "Version: 5\nSource: https://example.com/releases/\nMatching-Pattern: foo-@ANY_VERSION@@ARCHIVE_EXT@\n",
        )
        .unwrap();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].source, "https://example.com/releases/");
        assert_eq!(
            sources[0].matching_pattern,
            "foo-@ANY_VERSION@@ARCHIVE_EXT@"
        );
    }

    #[test]
    fn test_parse_v5_defaults_apply() {
        let sources = parse_watch(
            "Version: 5\n\
             Uversion-Mangle: s/RC/~rc/\n\
             Source: https://example.com/a/\nMatching-Pattern: a-@ANY_VERSION@\n\
             \n\
             Source: https://example.com/b/\nMatching-Pattern: b-@ANY_VERSION@\n",
        )
        .unwrap();
        assert_eq!(sources.len(), 2);
        assert_eq!(sources[1].uversion_mangle.as_deref(), Some("s/RC/~rc/"));
    }

    #[test]
    fn test_parse_v5_untrackable() {
        let sources =
            parse_watch("Version: 5\nUntrackable: on hold\nSource: https://example.com/\n")
                .unwrap();
        assert_eq!(sources[0].untrackable.as_deref(), Some("on hold"));
    }

    #[test]
    fn test_parse_v5_dfsg() {
        let sources = parse_watch(
            "Version: 5\n\
             Source: https://example.com/\n\
             Matching-Pattern: foo-@ANY_VERSION@@ARCHIVE_EXT@\n\
             Dversion-Mangle: s/\\+dfsg\\d*$//\n\
             Repacksuffix: +dfsg\n",
        )
        .unwrap();
        assert_eq!(sources[0].repack_suffix.as_deref(), Some("+dfsg"));
        assert_eq!(
            sources[0].dversion_mangle.as_deref(),
            Some("s/\\+dfsg\\d*$//")
        );
    }

    #[test_case("version=1\nhttps://example.com/ foo\n"; "v1 rejected")]
    #[test_case("https://example.com/ foo\n"; "no version")]
    #[test_case(
        "Version: 5\nSource: https://example.com/\nMode: git\n";
        "unsupported mode"
    )]
    #[test_case(
        "Version: 5\nSource: https://example.com/\nVersion-Schema: group\n";
        "unsupported version schema"
    )]
    fn test_parse_rejects(content: &str) {
        assert!(parse_watch(content).is_err());
    }

    #[test]
    fn test_parse_v5_component() {
        let sources = parse_watch(
            "Version: 5\n\
             Source: https://example.com/a/\nMatching-Pattern: a-@ANY_VERSION@\n\
             \n\
             Source: https://example.com/b/\nMatching-Pattern: b-@ANY_VERSION@\nComponent: bar\n",
        )
        .unwrap();
        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0].component, None);
        assert_eq!(sources[1].component.as_deref(), Some("bar"));
    }
}
