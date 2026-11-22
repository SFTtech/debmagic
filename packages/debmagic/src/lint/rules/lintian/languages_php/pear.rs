//! Facts that the Rules porting lintian's `languages/php/pear` Check share:
//! the PEAR/PECL package files, the package type in `package.xml`, and the
//! `debian/rules` lines that call pkg-php-tools.

use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::context::SourceContext;
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::package_relation::PackageRelation;

pub(super) const PACKAGE_XML: &str = "package.xml";
pub(super) const PACKAGE2_XML: &str = "package2.xml";
pub(super) const CHANNEL_XML: &str = "channel.xml";
pub(super) const COMPOSER_JSON: &str = "composer.json";
pub(super) const RULES: &str = "debian/rules";

/// The Check's view of one Source: `None` from [`PearSource::new`] when no
/// `.php` file is shipped, as lintian skips the Check then.
pub(super) struct PearSource {
    /// `Build-Depends` of the source paragraph; empty when `debian/control`
    /// is missing or does not parse.
    pub(super) build_depends: PackageRelation,
    /// `package.xml` exists (as any Entry).
    pub(super) package_xml: bool,
    /// `package2.xml` exists (as any Entry).
    pub(super) package2_xml: bool,
    pub(super) channel_xml: bool,
    pub(super) composer_json: bool,
    /// The `<...release>` or `<bundle>` kind of a regular `package.xml`, as
    /// PEAR names it; `unknown` when none is found or the file is not one.
    pub(super) package_type: String,
}

impl PearSource {
    pub(super) fn new(ctx: &mut SourceContext<'_>) -> Option<Self> {
        let build_depends = ctx
            .debian_control()
            .and_then(|control| control.source().relation("Build-Depends"))
            .unwrap_or_default();
        let files = ctx.files()?;
        if !files
            .entries()
            .iter()
            .any(|entry| !entry.is_dir() && has_php_extension(entry.path()))
        {
            return None;
        }
        let package_type = match files.get(PACKAGE_XML) {
            Some(entry) if entry.is_regular_file() => files
                .read(entry)
                .map(|bytes| package_type(&String::from_utf8_lossy(&bytes)))
                .unwrap_or_else(|_| "unknown".to_string()),
            _ => "unknown".to_string(),
        };
        Some(Self {
            build_depends,
            package_xml: files.get(PACKAGE_XML).is_some(),
            package2_xml: files.get(PACKAGE2_XML).is_some(),
            channel_xml: files.get(CHANNEL_XML).is_some(),
            composer_json: files.get(COMPOSER_JSON).is_some(),
            package_type,
        })
    }

    /// `package.xml` or `package2.xml` exists.
    pub(super) fn is_pear(&self) -> bool {
        self.package_xml || self.package2_xml
    }

    /// Build-Depends satisfies `pkg-php-tools`.
    pub(super) fn has_pkg_php_tools(&self) -> bool {
        self.build_depends.satisfies("pkg-php-tools")
    }

    /// Build-Depends satisfies `pkg-php-tools` or `dh-sequence-phppear`: the
    /// PEAR packaging is set up, as lintian's PEAR branch tests it.
    pub(super) fn has_pear_builddep(&self) -> bool {
        self.has_pkg_php_tools() || self.build_depends.satisfies("dh-sequence-phppear")
    }
}

/// lintian's `/\.php$/i`: the path ends with `.php`, ignoring case.
fn has_php_extension(path: &Path) -> bool {
    path.as_os_str()
        .as_encoded_bytes()
        .to_ascii_lowercase()
        .ends_with(b".php")
}

static PHP_RELEASE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*<(php|extsrc|extbin|zendextsrc|zendextbin)release\s*/?>").expect("valid regex")
});
static BUNDLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*<bundle\s*/?>").expect("valid regex"));

/// lintian's wild guess of the PEAR package type, line by line.
fn package_type(text: &str) -> String {
    for line in text.split_inclusive('\n') {
        if let Some(captures) = PHP_RELEASE.captures(line) {
            return captures[1].to_string();
        }
        if BUNDLE.is_match(line) {
            return "bundle".to_string();
        }
    }
    "unknown".to_string()
}

/// The `debian/rules` lines that select pkg-php-tools addons and build
/// systems, as lintian reads them.
#[derive(Default)]
pub(super) struct RulesFacts {
    pub(super) buildsystem_phppear: bool,
    pub(super) addon_phppear: bool,
    pub(super) addon_phpcomposer: bool,
    pub(super) addon_php: bool,
}

/// `debian/rules` as an open file, when the Check reads it: a regular file
/// or hardlink. Symlinks are not followed.
pub(super) fn rules_facts(files: &FileTree, build_depends: &PackageRelation) -> Option<RulesFacts> {
    let entry = files.get(RULES)?;
    if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
        return None;
    }
    let bytes = files.read(entry).ok()?;
    let text = String::from_utf8_lossy(&bytes);

    static BUILDSYSTEM_PHPPEAR: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^\t\s*dh\s.*--buildsystem(?:=|\s+)(?:\S+,)*phppear(?:,\S+)*\s")
            .expect("valid regex")
    });
    static ADDON_PHPPEAR: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^\t\s*dh\s.*--with(?:=|\s+)(?:\S+,)*phppear(?:,\S+)*\s").expect("valid regex")
    });
    static ADDON_PHPCOMPOSER: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^\t\s*dh\s.*--with(?:=|\s+)(?:\S+,)*phpcomposer(?:,\S+)*\s")
            .expect("valid regex")
    });
    static ADDON_PHP: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^\t\s*dh\s.*--with(?:=|\s+)(?:\S+,)*php(?:,\S+)*\s").expect("valid regex")
    });
    static COMMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*#").expect("valid regex"));

    let mut facts = RulesFacts::default();
    for line in logical_lines(&text) {
        if COMMENT.is_match(&line) {
            continue;
        }
        facts.buildsystem_phppear |= BUILDSYSTEM_PHPPEAR.is_match(&line);
        facts.addon_phppear |=
            ADDON_PHPPEAR.is_match(&line) || build_depends.satisfies("dh-sequence-phppear");
        facts.addon_phpcomposer |=
            ADDON_PHPCOMPOSER.is_match(&line) || build_depends.satisfies("dh-sequence-phpcomposer");
        facts.addon_php |= ADDON_PHP.is_match(&line);
    }
    Some(facts)
}

/// Lines with a trailing backslash joined to the next one, as lintian's
/// `while ($line =~ s/\\$// && ...)` loop does.
fn logical_lines(text: &str) -> Vec<String> {
    let mut raw = text.split_inclusive('\n');
    let mut lines = Vec::new();
    while let Some(first) = raw.next() {
        let mut line = first.to_string();
        while let Some(stripped) = strip_continuation(&line) {
            line = stripped;
            match raw.next() {
                Some(next) => line.push_str(next),
                None => break,
            }
        }
        lines.push(line);
    }
    lines
}

/// Perl's `s/\\$//` where `$` also matches before a final newline.
fn strip_continuation(line: &str) -> Option<String> {
    if let Some(rest) = line.strip_suffix('\\') {
        return Some(rest.to_string());
    }
    let rest = line.strip_suffix("\\\n")?;
    Some(format!("{rest}\n"))
}
