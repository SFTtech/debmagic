//! Symbols control-file grammar from `Lintian::Check::Debian::Shlibs`.
//!
//! Soname headers, alternative templates, meta lines, and symbol definitions
//! are classified in that order. Tags that compare the file to an ELF SONAME
//! or the dynamic symbol table are not produced here.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Location;

use super::lines;

static SONAME_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([^\s|*]\S+)\s\S+\s*(?:\(\S+\s+\S+\)|#MINVER#)?").expect("soname line")
});
static ALTERNATIVE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\|\s+\S+\s*(?:\(\S+\s+\S+\)|#MINVER#)?").expect("alternative template")
});
static META: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\*\s(\S+):\s\S+").expect("meta line"));
static SYMBOL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s+(\S+)\s(\S+)(?:\s(\S+(?:\s\S+)?))?$").expect("symbol definition")
});
static DEPENDENCY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\S+)(\s*(?:\(\S+\s+\S+\)|#MINVER#))?$").expect("dependency"));
static COMMA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*,\s*").expect("comma"));
static PIPE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*\|\s*").expect("pipe"));
static REVISION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.+)-[^-]+$").expect("debian revision"));
static SO_VERSION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.*)\.so\.(.*)$").expect("lib.so.version"));
static DASH_SO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.*)-(\d.*)\.so$").expect("lib-version.so"));

pub(super) struct Findings {
    pub syntax_errors: Vec<u32>,
    pub invalid_templates: Vec<(u32, String)>,
    pub duplicates: Vec<String>,
    pub current_versions: Vec<String>,
    pub debian_revisions: Vec<String>,
}

pub(super) fn at(line: Option<u32>) -> Location {
    Location {
        path: "symbols".into(),
        line,
        column: None,
    }
}

pub(super) fn inspect(ctx: &mut BinaryPackageContext<'_>) -> Option<Findings> {
    let version = package_version(ctx);
    let text = lines::read_text(ctx, "symbols")?;
    Some(parse(&text, &version))
}

fn package_version(ctx: &BinaryPackageContext<'_>) -> String {
    ctx.installation_control()
        .fields()
        .get("Version")
        .filter(|value| !value.is_empty())
        .unwrap_or("0-1")
        .to_string()
}

pub(super) fn parse(text: &str, package_version: &str) -> Findings {
    let package_version_wo_rev = version_without_revision(package_version);
    let package_has_revision = package_version.contains('-');

    let mut syntax_errors = Vec::new();
    let mut invalid_templates = Vec::new();
    let mut positions: HashMap<String, Vec<u32>> = HashMap::new();
    let mut sonames = Vec::new();
    let mut symbols_by_soname: HashMap<String, Vec<String>> = HashMap::new();
    let mut full_version: HashMap<String, Vec<String>> = HashMap::new();
    let mut debian_revision: HashMap<String, Vec<String>> = HashMap::new();
    let mut meta_seen = HashSet::new();
    let mut current_soname = String::new();
    let mut template_count = 0u32;

    for (index, line) in lines::perl_lines(text).into_iter().enumerate() {
        let position = index as u32 + 1;
        if lines::is_blank_or_comment(line) {
            continue;
        }

        if let Some(caps) = SONAME_LINE.captures(line) {
            current_soname = caps[1].to_string();
            sonames.push(current_soname.clone());
            positions
                .entry(current_soname.clone())
                .or_default()
                .push(position);
            let rest = line[current_soname.len()..].trim_start();
            if dependencies_malformed(rest) {
                syntax_errors.push(position);
            }
            template_count = 0;
            continue;
        }

        if ALTERNATIVE.is_match(line) {
            let mut error = current_soname.is_empty() || meta_seen.contains(&current_soname);
            if error {
                syntax_errors.push(position);
            }
            let rest = line.strip_prefix('|').unwrap_or(line).trim_start();
            if dependencies_malformed(rest) {
                if !error {
                    syntax_errors.push(position);
                }
                error = true;
            }
            if !error {
                template_count += 1;
            }
            continue;
        }

        if META.is_match(line) {
            let has_symbols = symbols_by_soname
                .get(&current_soname)
                .is_some_and(|symbols| !symbols.is_empty());
            if has_symbols {
                syntax_errors.push(position);
            }
            meta_seen.insert(current_soname.clone());
            continue;
        }

        if let Some(caps) = SYMBOL.captures(line) {
            let symbol = caps[1].to_string();
            let version = caps[2].to_string();
            let selector = caps.get(3).map(|group| group.as_str()).unwrap_or("");
            if current_soname.is_empty() {
                syntax_errors.push(position);
            }
            symbols_by_soname
                .entry(current_soname.clone())
                .or_default()
                .push(symbol.clone());
            if version == package_version && package_has_revision {
                full_version
                    .entry(current_soname.clone())
                    .or_default()
                    .push(symbol);
            } else if version.contains('-')
                && !version.ends_with('~')
                && version != package_version_wo_rev
            {
                debian_revision
                    .entry(current_soname.clone())
                    .or_default()
                    .push(symbol);
            }
            if template_id_invalid(selector, template_count) {
                invalid_templates.push((position, selector.to_string()));
            }
            continue;
        }

        syntax_errors.push(position);
    }

    Findings {
        syntax_errors: uniq(syntax_errors),
        invalid_templates,
        duplicates: lines::duplicate_groups(&positions, human_soname),
        current_versions: version_messages(&sonames, &full_version),
        debian_revisions: version_messages(&sonames, &debian_revision),
    }
}

fn dependencies_malformed(rest: &str) -> bool {
    let conjunctives = lines::perl_split(&COMMA, rest);
    if conjunctives.is_empty() {
        return false;
    }
    for conjunctive in conjunctives {
        let disjunctives = lines::perl_split(&PIPE, conjunctive);
        if disjunctives.is_empty() {
            return true;
        }
        for disjunctive in disjunctives {
            if disjunctive.is_empty() || !DEPENDENCY.is_match(disjunctive) {
                return true;
            }
        }
    }
    false
}

fn template_id_invalid(selector: &str, template_count: u32) -> bool {
    if selector.is_empty() {
        return false;
    }
    match selector.parse::<u128>() {
        Ok(id) => id > u128::from(template_count),
        Err(_) => true,
    }
}

fn version_without_revision(version: &str) -> String {
    REVISION
        .captures(version)
        .map(|caps| caps[1].to_string())
        .unwrap_or_else(|| version.to_string())
}

/// `libfoo.so.1` → `libfoo 1`, then `libfoo-1.2.so` → `libfoo 1.2`.
fn human_soname(soname: &str) -> String {
    if let Some(caps) = SO_VERSION.captures(soname) {
        return format!("{} {}", &caps[1], &caps[2]);
    }
    if let Some(caps) = DASH_SO.captures(soname) {
        return format!("{} {}", &caps[1], &caps[2]);
    }
    soname.to_string()
}

fn version_messages(sonames: &[String], symbols: &HashMap<String, Vec<String>>) -> Vec<String> {
    uniq_strings(sonames)
        .into_iter()
        .filter_map(|soname| {
            let found = symbols.get(&soname)?;
            if found.is_empty() {
                return None;
            }
            Some(version_context(found, &soname))
        })
        .collect()
}

fn version_context(symbols: &[String], soname: &str) -> String {
    let unique = uniq_strings(symbols);
    let mut sorted = unique;
    sorted.sort();
    let mut context = format!("on symbol {}", sorted[0]);
    if sorted.len() > 1 {
        context.push_str(&format!(" and {} others", sorted.len() - 1));
    }
    format!("{context} ({soname})")
}

fn uniq(items: Vec<u32>) -> Vec<u32> {
    let mut seen = HashSet::new();
    items
        .into_iter()
        .filter(|item| seen.insert(*item))
        .collect()
}

fn uniq_strings(items: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    items
        .iter()
        .filter(|item| seen.insert((*item).clone()))
        .cloned()
        .collect()
}

#[cfg(test)]
pub(super) fn symbols_case(
    version: Option<&str>,
    body: &str,
) -> crate::lint::tester::BinaryPackageCase {
    let mut case = crate::lint::tester::BinaryPackageCase::from(Vec::<(
        &str,
        crate::lint::tester::TestFile,
    )>::new())
    .control_file("symbols", crate::lint::tester::TestFile::new(body));
    if let Some(version) = version {
        case = case.field("Version", version);
    }
    case
}

#[cfg(test)]
pub(super) const LEGACY_DEBUG: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/lintian-parity/checks/debian/shlibs/legacy-debug/debian/_symbols"
));

#[cfg(test)]
pub(super) const LIBFOO: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/lintian-parity/checks/debian/shlibs/shared-libs-symbols-file/debian/libfoo1.symbols"
));

#[cfg(test)]
pub(super) const LIBBAZ: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/lintian-parity/checks/debian/shlibs/legacy-libbaz/debian/lib.symbols"
));

#[cfg(test)]
impl std::fmt::Debug for Findings {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Findings")
            .field("syntax_errors", &self.syntax_errors)
            .field("invalid_templates", &self.invalid_templates)
            .field("duplicates", &self.duplicates)
            .field("current_versions", &self.current_versions)
            .field("debian_revisions", &self.debian_revisions)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_debug_matches_the_recipe() {
        let found = parse(LEGACY_DEBUG, "1.0");
        assert_eq!(found.syntax_errors, vec![1, 3, 4, 11, 15, 19]);
        assert_eq!(
            found.invalid_templates,
            vec![
                (4, "should this".to_string()),
                (12, "4".to_string()),
                (14, "A".to_string()),
            ]
        );
        assert_eq!(
            found.duplicates,
            vec!["(lines 6 22) libhello 0".to_string()]
        );
        assert!(found.current_versions.is_empty());
        assert!(found.debian_revisions.is_empty());
    }

    #[test]
    fn libfoo_matches_the_recipe() {
        let found = parse(LIBFOO, "1.0-1");
        assert_eq!(found.syntax_errors, vec![10]);
        assert_eq!(found.invalid_templates, vec![(9, "1".to_string())]);
        assert_eq!(found.duplicates, vec!["(lines 1 8) libfoo 1".to_string()]);
        assert_eq!(
            found.current_versions,
            vec!["on symbol e@Base (libfoo.so.1)".to_string()]
        );
        assert_eq!(
            found.debian_revisions,
            vec!["on symbol energy@Base (libfoo.so.1)".to_string()]
        );
    }

    #[test]
    fn libbaz_versions_match_the_recipe() {
        let found = parse(LIBBAZ, "1-1");
        assert!(found.syntax_errors.is_empty());
        assert_eq!(
            found.current_versions,
            vec!["on symbol pw (libbaz.so.2)".to_string()]
        );
        assert_eq!(
            found.debian_revisions,
            vec!["on symbol foo (libbaz.so.2)".to_string()]
        );
    }

    #[test]
    fn human_soname_splits_both_layouts() {
        assert_eq!(human_soname("libfoo.so.1"), "libfoo 1");
        assert_eq!(human_soname("libfoo-1.2.so"), "libfoo 1.2");
        assert_eq!(human_soname("plugin.so"), "plugin.so");
    }

    #[test]
    fn tilde_and_upstream_versions_are_quiet() {
        let text = "\
libfoo.so.1 pkg #MINVER#
 sym@Base 1.0
 other@Base 1.0-3~
";
        let found = parse(text, "1.0-1");
        assert!(found.current_versions.is_empty());
        assert!(found.debian_revisions.is_empty());
        assert!(found.syntax_errors.is_empty());
    }

    #[test]
    fn several_symbols_name_the_first_sorted_one() {
        let text = "\
libfoo.so.1 pkg #MINVER#
 b@Base 1.0-1
 a@Base 1.0-1
 b@Base 1.0-1
";
        let found = parse(text, "1.0-1");
        assert_eq!(
            found.current_versions,
            vec!["on symbol a@Base and 1 others (libfoo.so.1)".to_string()]
        );
    }

    #[test]
    fn missing_package_version_defaults_to_0_1() {
        let text = "\
libfoo.so.1 pkg #MINVER#
 sym@Base 0-1
";
        let found = parse(text, "0-1");
        assert_eq!(
            found.current_versions,
            vec!["on symbol sym@Base (libfoo.so.1)".to_string()]
        );
    }

    #[test]
    fn template_id_equal_to_the_count_is_valid() {
        let text = "\
libfoo.so.1 pkg #MINVER#
| other
 sym@Base 1.0 1
 sym@Base 1.0 0
";
        let found = parse(text, "1.0");
        assert!(found.invalid_templates.is_empty(), "{found:?}");
        assert!(found.syntax_errors.is_empty(), "{found:?}");
    }
}
