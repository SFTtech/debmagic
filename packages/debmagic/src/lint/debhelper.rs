//! Debhelper facts for a source packaging tree.
//!
//! Lintian's `debhelper` Check reads the declared compat level from
//! `dh_assistant active-compat-level` (`declared-compat-level`, not the
//! `DH_COMPAT` environment override) and only then emits compat Tags, and
//! only after `debian/rules` has shown a dh command. This module reproduces
//! that without running `dh_assistant`: the compat file, `Build-Depends:
//! debhelper-compat (= N)`, and `X-DH-Compat`. A make-level `DH_COMPAT` in
//! `debian/rules` is a different fact (the conflict Tag) and is not stored
//! here. dh addons are not stored here.

use std::path::{Component, Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use super::debian_control::DebianControl;
use super::file_tree::{EntryKind, FileTree};

/// Level below which lintian 2.139.0 emits
/// `package-uses-deprecated-debhelper-compat-version`
/// (`data/debhelper/compat-level`, `deprecated`).
pub(crate) const DEPRECATED_COMPAT_LEVEL: u32 = 10;

/// `debian/compat` at 14 or above makes `dh_assistant` abort: 15+ is rejected
/// outright, and 14 is rejected once `HIGHEST_STABLE_COMPAT_LEVEL` is above 13
/// (debhelper's constant is 14).
const COMPAT_FILE_REJECTED_AT: u32 = 14;

static COMMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*#").expect("regex"));
static DH_COMMAND: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s+[@+-]?(?:\S+=\S+\s+)*(dh_\S+)").expect("regex"));
static DH_SEQUENCER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:\s+[@+-]?(?:\S+=\S+\s+)*)dh\s+").expect("regex"));
static INDIRECT_DH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^include\s+/usr/share/(?:dh-php/pkg-pecl\.mk|blends-dev/rules)").expect("regex")
});
static EXACT_COMPAT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^debhelper-compat\s*\(\s*=\s*([^\s)]+)\s*\)$").expect("regex"));

/// Where the declared compat level came from. Absent when nothing is declared
/// (dh_assistant then reports level 1 and a null source) or when determining
/// the level would abort.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CompatSource {
    DebianCompat,
    BuildDepends(u32),
    XDhCompat(u32),
}

/// Whether this packaging tree uses debhelper, and the declared compat level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DebhelperFacts {
    uses_debhelper: bool,
    /// `None` when `dh_assistant` would abort (empty or invalid compat file,
    /// conflicting declarations, control dh_assistant cannot parse). `Some(1)`
    /// when nothing is declared. `Some(0)` is a parsed compat file of `0`;
    /// [`Self::level`] turns that into 1, as Perl `||=` does.
    declared_level: Option<u32>,
    source: Option<CompatSource>,
}

enum PackagingFile {
    Missing,
    Unreadable,
    Text(String),
}

impl DebhelperFacts {
    /// Facts for the packaging tree. `None` only when the File tree itself is
    /// unavailable.
    pub(crate) fn from_tree(control: Option<&DebianControl>, files: &FileTree) -> Self {
        Self::inspect(
            control,
            read_packaging(files, "debian/rules"),
            read_packaging(files, "debian/compat"),
        )
    }

    fn inspect(
        control: Option<&DebianControl>,
        rules: PackagingFile,
        compat: PackagingFile,
    ) -> Self {
        let driver = control
            .and_then(|control| control.source().get("Build-Driver"))
            .map(str::trim);
        if driver.is_some_and(|driver| driver != "debian-rules") {
            return Self::unused();
        }
        let PackagingFile::Text(rules) = rules else {
            return Self::unused();
        };
        if !uses_any_dh_command(&rules) {
            return Self::unused();
        }
        let (declared_level, source) = declared_compat(control, compat);
        Self {
            uses_debhelper: true,
            declared_level,
            source,
        }
    }

    fn unused() -> Self {
        Self {
            uses_debhelper: false,
            declared_level: None,
            source: None,
        }
    }

    pub(crate) fn uses_debhelper(&self) -> bool {
        self.uses_debhelper
    }

    /// Declared level after Perl `||= 1`: abort and `0` both become 1.
    pub(crate) fn level(&self) -> u32 {
        match self.declared_level {
            Some(level) if level > 0 => level,
            _ => 1,
        }
    }

    #[cfg(test)]
    fn compat_source(&self) -> Option<&CompatSource> {
        self.source.as_ref()
    }
}

fn declared_compat(
    control: Option<&DebianControl>,
    compat: PackagingFile,
) -> (Option<u32>, Option<CompatSource>) {
    let Some(control) = control else {
        return (None, None);
    };
    let Ok(from_control) = control_compat(control) else {
        return (None, None);
    };
    match compat {
        PackagingFile::Unreadable => (None, None),
        PackagingFile::Text(text) => match compat_file_level(&text) {
            Err(()) => (None, None),
            Ok(level)
                if from_control.build_depends.is_some()
                    || from_control.x_dh_compat.is_some()
                    || level >= COMPAT_FILE_REJECTED_AT =>
            {
                (None, None)
            }
            Ok(level) => (Some(level), Some(CompatSource::DebianCompat)),
        },
        PackagingFile::Missing => {
            if let Some(level) = from_control.build_depends {
                (Some(level), Some(CompatSource::BuildDepends(level)))
            } else if let Some(level) = from_control.x_dh_compat {
                (Some(level), Some(CompatSource::XDhCompat(level)))
            } else {
                (Some(1), None)
            }
        }
    }
}

struct ControlCompat {
    build_depends: Option<u32>,
    x_dh_compat: Option<u32>,
}

fn control_compat(control: &DebianControl) -> Result<ControlCompat, ()> {
    let source = control.source();
    if !valid_source_name(source.get("Source").unwrap_or("").trim()) {
        return Err(());
    }
    let x_dh_compat = match source.get("X-DH-Compat") {
        None => None,
        Some(value) if value.contains('\n') => return Err(()),
        Some(value) => Some(positive_integer(value.trim())?),
    };
    let mut build_depends = None;
    for field in ["Build-Depends", "Build-Depends-Arch", "Build-Depends-Indep"] {
        let Some(value) = source.get(field) else {
            continue;
        };
        for dep in relations(value) {
            let Some(level) = debhelper_compat_relation(&dep)? else {
                continue;
            };
            if field != "Build-Depends" || build_depends.is_some() {
                return Err(());
            }
            build_depends = Some(level);
        }
    }
    if build_depends.is_some() && x_dh_compat.is_some() {
        return Err(());
    }
    Ok(ControlCompat {
        build_depends,
        x_dh_compat,
    })
}

/// `Ok(None)` when the dependency is not `debhelper-compat`. `Err` when it is
/// present but not an exact integer `debhelper-compat (= N)`.
fn debhelper_compat_relation(dep: &str) -> Result<Option<u32>, ()> {
    let dep = dep.trim();
    if !dep.starts_with("debhelper-compat") {
        return Ok(None);
    }
    let Some(captures) = EXACT_COMPAT.captures(dep) else {
        return Err(());
    };
    let version = &captures[1];
    if !version.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(());
    }
    Ok(Some(positive_integer(version)?))
}

fn relations(value: &str) -> Vec<String> {
    let flat = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let flat = flat.trim().trim_end_matches(',').trim();
    if flat.is_empty() {
        return Vec::new();
    }
    flat.split(',')
        .map(str::trim)
        .filter(|dep| !dep.is_empty())
        .map(str::to_string)
        .collect()
}

fn positive_integer(text: &str) -> Result<u32, ()> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(());
    }
    text.parse().map_err(|_| ())
}

fn compat_file_level(text: &str) -> Result<u32, ()> {
    let line = text.split('\n').next().unwrap_or("").trim_end_matches('\r');
    positive_integer(line.trim())
}

/// lintian's `$PKGNAME_REGEX`: `[a-z0-9][-+.a-z0-9]+`.
fn valid_source_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() || first.is_ascii_digit() => {}
        _ => return false,
    }
    let mut rest = 0usize;
    for ch in chars {
        if !(ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '-' | '+' | '.')) {
            return false;
        }
        rest += 1;
    }
    rest >= 1
}

/// `dh_autoreconf` is excluded by lintian's `(?!autoreconf)` lookahead.
fn is_counted_dh_command(line: &str) -> bool {
    DH_COMMAND
        .captures(line)
        .is_some_and(|captures| !captures[1].starts_with("dh_autoreconf"))
}

fn uses_any_dh_command(text: &str) -> bool {
    let lines: Vec<&str> = text
        .split('\n')
        .map(|line| line.trim_end_matches('\r'))
        .collect();
    let mut index = 0;
    while index < lines.len() {
        let mut line = lines[index].to_string();
        index += 1;
        while line.ends_with('\\') {
            line.pop();
            if index >= lines.len() {
                break;
            }
            line.push_str(lines[index]);
            index += 1;
        }
        if COMMENT.is_match(&line) {
            continue;
        }
        if is_counted_dh_command(&line)
            || DH_SEQUENCER.is_match(&line)
            || INDIRECT_DH.is_match(&line)
        {
            return true;
        }
    }
    false
}

fn read_packaging(files: &FileTree, path: &str) -> PackagingFile {
    let mut path = PathBuf::from(path);
    for _ in 0..8 {
        let Some(entry) = files.get(&path) else {
            return PackagingFile::Missing;
        };
        match entry.kind() {
            EntryKind::Directory => return PackagingFile::Unreadable,
            EntryKind::RegularFile | EntryKind::Hardlink => {
                return match files.read(entry) {
                    Ok(bytes) => PackagingFile::Text(String::from_utf8_lossy(&bytes).into_owned()),
                    Err(_) => PackagingFile::Unreadable,
                };
            }
            EntryKind::Symlink => {
                let Some(target) = entry.link_target() else {
                    return PackagingFile::Unreadable;
                };
                let Some(next) = resolve_link(&path, target) else {
                    return PackagingFile::Unreadable;
                };
                path = next;
            }
        }
    }
    PackagingFile::Unreadable
}

fn resolve_link(from: &Path, target: &Path) -> Option<PathBuf> {
    if target.is_absolute() {
        return None;
    }
    let mut resolved = from.parent().unwrap_or(Path::new("")).to_path_buf();
    for component in target.components() {
        match component {
            Component::Normal(part) => resolved.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                if !resolved.pop() {
                    return None;
                }
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn control(text: &str) -> DebianControl {
        DebianControl::parse(text).expect("control")
    }

    const CONTROL: &str = "\
Source: example\n\
Maintainer: Example <ex@example.com>\n\
Standards-Version: 4.7.2\n\
\n\
Package: example\n\
Architecture: all\n\
Description: example\n extra\n";

    fn with_rules(rules: &str, compat: PackagingFile) -> DebhelperFacts {
        DebhelperFacts::inspect(
            Some(&control(CONTROL)),
            PackagingFile::Text(rules.into()),
            compat,
        )
    }

    #[test]
    fn dh_sequencer_and_commands_count() {
        let rules = "#!/usr/bin/make -f\n%:\n\tdh $@\n";
        let facts = with_rules(rules, PackagingFile::Text("9\n".into()));
        assert!(facts.uses_debhelper());
        assert_eq!(facts.level(), 9);
        assert_eq!(facts.compat_source(), Some(&CompatSource::DebianCompat));
    }

    #[test]
    fn nothing_declared_is_level_one_without_a_source() {
        let facts = with_rules("\tdh $@\n", PackagingFile::Missing);
        assert_eq!(facts.declared_level, Some(1));
        assert_eq!(facts.level(), 1);
        assert_eq!(facts.compat_source(), None);
    }

    #[test]
    fn empty_compat_file_aborts_to_level_one() {
        let facts = with_rules("\tdh $@\n", PackagingFile::Text("\n".into()));
        assert_eq!(facts.declared_level, None);
        assert_eq!(facts.level(), 1);
    }

    #[test]
    fn compat_zero_is_false_under_perl_or_assign() {
        let facts = with_rules("\tdh $@\n", PackagingFile::Text("0\n".into()));
        assert_eq!(facts.declared_level, Some(0));
        assert_eq!(facts.level(), 1);
    }

    #[test]
    fn build_depends_exact_integer() {
        let text = "\
Source: example\n\
Build-Depends: debhelper,\n debhelper-compat (= 13)\n\
\n\
Package: example\n\
Architecture: all\n\
Description: example\n extra\n";
        let facts = DebhelperFacts::inspect(
            Some(&control(text)),
            PackagingFile::Text("\tdh $@\n".into()),
            PackagingFile::Missing,
        );
        assert_eq!(facts.level(), 13);
        assert_eq!(facts.compat_source(), Some(&CompatSource::BuildDepends(13)));
    }

    #[test]
    fn x_dh_compat() {
        let text = "\
Source: example\n\
X-DH-Compat: 9\n\
\n\
Package: example\n\
Architecture: all\n\
Description: example\n extra\n";
        let facts = DebhelperFacts::inspect(
            Some(&control(text)),
            PackagingFile::Text("\tdh_clean\n".into()),
            PackagingFile::Missing,
        );
        assert_eq!(facts.level(), 9);
        assert_eq!(facts.compat_source(), Some(&CompatSource::XDhCompat(9)));
    }

    #[test]
    fn dh_autoreconf_and_comments_do_not_count() {
        let rules = "\
#\tdh $@\n\
\tdh_autoreconf\n\
\tdh_autoreconf_clean\n";
        let facts = with_rules(rules, PackagingFile::Missing);
        assert!(!facts.uses_debhelper());
    }

    #[test]
    fn indirect_include_counts() {
        let facts = with_rules(
            "include /usr/share/dh-php/pkg-pecl.mk\n",
            PackagingFile::Missing,
        );
        assert!(facts.uses_debhelper());
        assert_eq!(facts.level(), 1);
    }

    #[test]
    fn other_build_driver_is_not_debhelper() {
        let text = "\
Source: example\n\
Build-Driver: debputy\n\
\n\
Package: example\n\
Architecture: all\n\
Description: example\n extra\n";
        let facts = DebhelperFacts::inspect(
            Some(&control(text)),
            PackagingFile::Text("\tdh $@\n".into()),
            PackagingFile::Missing,
        );
        assert!(!facts.uses_debhelper());
    }
}
