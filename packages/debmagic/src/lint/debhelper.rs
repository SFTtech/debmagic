//! Debhelper facts for a source packaging tree.
//!
//! Lintian's `debhelper` Check reads the declared compat level from
//! `dh_assistant active-compat-level` (`declared-compat-level`, not the
//! `DH_COMPAT` environment override) and only then emits compat Tags, and
//! only after `debian/rules` has shown a dh command. This module reproduces
//! that without running `dh_assistant`: the compat file, `Build-Depends:
//! debhelper-compat (= N)`, and `X-DH-Compat`. A make-level `DH_COMPAT` in
//! `debian/rules` is a different fact (the conflict Tag) and is not stored
//! here. Whether `debian/compat` could be read is stored.
//!
//! The `dh_*` command lines of `debian/rules` are stored as lintian's walk
//! sees them, with whether a make conditional surrounds them; [`prerequisites`]
//! maps a command to the build dependency that provides it. The `--with`
//! addons of `dh` lines are stored the same way. `dh-sequence-*` build
//! dependencies are not stored here yet.

use std::path::{Component, Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use super::debian_control::DebianControl;
use super::file_tree::{EntryKind, FileTree};

pub(crate) mod prerequisites;

/// First level that can be declared with `debhelper-compat` (11.1.5~alpha1).
pub(crate) const VERSIONED_PREREQUISITE_AVAILABLE: u64 = 11;

/// lintian 2.139.0 `data/debhelper/compat-level`.
static COMPAT_LEVELS: LazyLock<Vec<(&'static str, u64)>> = LazyLock::new(|| {
    crate::lint::data::lines(include_str!("data/debhelper/compat-level"))
        .filter_map(|line| {
            let (key, value) = crate::lint::data::assignment(line)?;
            Some((key, value.parse().ok()?))
        })
        .collect()
});

fn compat_level(key: &str) -> u64 {
    COMPAT_LEVELS
        .iter()
        .find(|(name, _)| *name == key)
        .map(|(_, level)| *level)
        .unwrap_or_else(|| panic!("data/debhelper/compat-level missing {key}"))
}

/// Level below which lintian 2.139.0 emits
/// `package-uses-deprecated-debhelper-compat-version`
/// (`data/debhelper/compat-level`, `deprecated`).
pub(crate) fn deprecated_compat_level() -> u32 {
    u32::try_from(compat_level("deprecated")).expect("deprecated compat level fits u32")
}

/// `data/debhelper/compat-level`, `experimental`.
pub(crate) fn experimental_compat_level() -> u64 {
    compat_level("experimental")
}

/// `debian/compat` at 14 or above makes `dh_assistant` abort: 15+ is rejected
/// outright, and 14 is rejected once `HIGHEST_STABLE_COMPAT_LEVEL` is above 13
/// (debhelper's constant is 14).
const COMPAT_FILE_REJECTED_AT: u32 = 14;

static COMMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*#").expect("regex"));
static DH_COMMAND: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s+[@+-]?(?:\S+=\S+\s+)*(dh_\S+)").expect("regex"));
static DH_SEQUENCER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:\s+[@+-]?(?:\S+=\S+\s+)*)dh\s+").expect("regex"));
/// `--with` and its list, as lintian's `dh` branch matches it: a quoted list
/// first (group 1 or 2), else the bare word (group 3).
static WITH_ADDONS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\s--with(?:=|\s+)(?:'(\S+)'|"(\S+)"|(\S+))"#).expect("regex"));
static MAKE_CONDITIONAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^ifn?(?:eq|def)\s").expect("regex"));
static MAKE_ENDIF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^endif\s").expect("regex"));
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

/// A `dh_*` command at the start of a recipe line in `debian/rules`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DhCommand {
    name: String,
    in_make_conditional: bool,
}

impl DhCommand {
    /// The command as written, e.g. `dh_installdocs`.
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// lintian's `$maybe_skipping`: more `ifeq`/`ifneq`/`ifdef`/`ifndef`
    /// lines than `endif` lines came before, or fewer (a stray `endif` counts
    /// too).
    pub(crate) fn in_make_conditional(&self) -> bool {
        self.in_make_conditional
    }
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
    /// Numeric value of the first line of a readable `debian/compat`, as
    /// Perl reads it (leading digits, else 0). `None` when the file is
    /// missing, unreadable, or empty after trimming.
    compat_file_value: Option<u64>,
    /// `debian/compat` was opened and read, including an empty file.
    /// Meaningful when [`Self::uses_debhelper`] is true; otherwise the file
    /// was not inspected.
    compat_file_open: bool,
    /// `dh_*` command lines in file order, repeats kept. Empty when
    /// [`Self::uses_debhelper`] is false.
    commands: Vec<DhCommand>,
    /// `--with` addons of `dh` lines in file order, repeats kept, with `-`
    /// read as `_` (lintian's `$addon`). Not limited to make conditionals.
    /// Empty when [`Self::uses_debhelper`] is false.
    addons: Vec<String>,
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
        let walk = walk_rules(&rules);
        if !walk.seen_any_dh_command {
            return Self::unused();
        }
        let compat_file_open = matches!(compat, PackagingFile::Text(_));
        let compat_file_value = match &compat {
            PackagingFile::Text(text) => compat_file_numeric(text),
            _ => None,
        };
        let (declared_level, source) = declared_compat(control, compat);
        Self {
            uses_debhelper: true,
            declared_level,
            source,
            compat_file_value,
            compat_file_open,
            commands: walk.commands,
            addons: walk.addons,
        }
    }

    fn unused() -> Self {
        Self {
            uses_debhelper: false,
            declared_level: None,
            source: None,
            compat_file_value: None,
            compat_file_open: false,
            commands: Vec::new(),
            addons: Vec::new(),
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

    /// The level `dh_assistant` reports as declared, before Perl `||= 1`.
    /// `None` when it would abort or nothing is declared; `Some(0)` is a
    /// compat file of `0`.
    pub(crate) fn declared_level(&self) -> Option<u32> {
        self.source.as_ref().and(self.declared_level)
    }

    /// Numeric value of the `debian/compat` first line, see the field.
    pub(crate) fn compat_file_value(&self) -> Option<u64> {
        self.compat_file_value
    }

    /// `debian/compat` was opened, including when the contents are empty.
    /// See the field for when this is meaningful.
    pub(crate) fn compat_file_open(&self) -> bool {
        self.compat_file_open
    }

    /// Where the declared level came from. `None` when nothing was declared
    /// or determining the level would abort.
    pub(crate) fn compat_source(&self) -> Option<&CompatSource> {
        self.source.as_ref()
    }

    /// `dh_*` command lines of `debian/rules` in file order, as lintian's
    /// `debhelper` Check matches them (`dh_autoreconf*` excluded). Empty when
    /// debhelper is not used.
    pub(crate) fn commands(&self) -> &[DhCommand] {
        &self.commands
    }

    /// `--with` addons of `dh` lines in file order, repeats kept, with `-`
    /// read as `_`. Empty when debhelper is not used.
    pub(crate) fn addons(&self) -> &[String] {
        &self.addons
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

/// First line of `debian/compat`, trimmed, as a Perl number: leading digits,
/// else 0. `None` when the trimmed line is empty.
fn compat_file_numeric(text: &str) -> Option<u64> {
    let line = text.split('\n').next().unwrap_or("");
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let digits: String = line.chars().take_while(char::is_ascii_digit).collect();
    Some(
        digits
            .parse()
            .unwrap_or(if digits.is_empty() { 0 } else { u64::MAX }),
    )
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

/// The `dh_*` command a recipe line starts with. `dh_autoreconf*` is
/// excluded by lintian's `(?!autoreconf)` lookahead.
fn counted_dh_command(line: &str) -> Option<&str> {
    let name = DH_COMMAND.captures(line)?.get(1)?.as_str();
    (!name.starts_with("dh_autoreconf")).then_some(name)
}

/// The `--with` addons a `dh` line names, split on `,`, with `-` read as `_`.
/// Empty pieces are dropped: lintian's empty addon has no prerequisite.
fn with_addons(line: &str) -> Vec<String> {
    WITH_ADDONS
        .captures_iter(line)
        .filter_map(|captures| captures.get(1).or(captures.get(2)).or(captures.get(3)))
        .flat_map(|list| list.as_str().split(','))
        .filter(|addon| !addon.is_empty())
        .map(|addon| addon.replace('-', "_"))
        .collect()
}

/// What one walk over `debian/rules` records.
struct RulesWalk {
    /// lintian's `$seen_any_dh_command`.
    seen_any_dh_command: bool,
    commands: Vec<DhCommand>,
    addons: Vec<String>,
}

/// Walk `debian/rules` as lintian's `debhelper` Check reads it: a line ends
/// after `\n` and keeps it, a backslash at the end (before that newline)
/// joins the next line with the newline kept, make conditionals are counted,
/// and only then are comment lines dropped.
fn walk_rules(text: &str) -> RulesWalk {
    let physical: Vec<&str> = text.split_inclusive('\n').collect();
    let mut walk = RulesWalk {
        seen_any_dh_command: false,
        commands: Vec::new(),
        addons: Vec::new(),
    };
    let mut maybe_skipping = 0i64;
    let mut index = 0;
    while index < physical.len() {
        let mut line = physical[index].to_string();
        index += 1;
        while let Some(stripped) = strip_continuation(&line) {
            line = stripped;
            let Some(next) = physical.get(index) else {
                break;
            };
            line.push_str(next);
            index += 1;
        }
        if MAKE_CONDITIONAL.is_match(&line) {
            maybe_skipping += 1;
        } else if MAKE_ENDIF.is_match(&line) {
            maybe_skipping -= 1;
        }
        if COMMENT.is_match(&line) {
            continue;
        }
        if let Some(name) = counted_dh_command(&line) {
            walk.commands.push(DhCommand {
                name: name.to_string(),
                in_make_conditional: maybe_skipping != 0,
            });
            walk.seen_any_dh_command = true;
        } else if DH_SEQUENCER.is_match(&line) {
            walk.seen_any_dh_command = true;
            walk.addons.extend(with_addons(&line));
        } else if INDIRECT_DH.is_match(&line) {
            walk.seen_any_dh_command = true;
        }
    }
    walk
}

/// Perl `s/\\$//`: one backslash at the very end, or just before a final
/// newline, is dropped; the newline stays.
fn strip_continuation(line: &str) -> Option<String> {
    match line.strip_suffix('\n') {
        Some(body) => body.strip_suffix('\\').map(|body| format!("{body}\n")),
        None => line.strip_suffix('\\').map(str::to_string),
    }
}

fn read_packaging(files: &FileTree, path: &str) -> PackagingFile {
    let mut path = PathBuf::from(path);
    for _ in 0..8 {
        let Some(entry) = files.get(&path) else {
            return PackagingFile::Missing;
        };
        match entry.kind() {
            EntryKind::Directory
            | EntryKind::Fifo
            | EntryKind::CharDevice
            | EntryKind::BlockDevice => return PackagingFile::Unreadable,
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
        assert!(!facts.compat_file_open());
    }

    #[test]
    fn empty_compat_file_aborts_to_level_one() {
        let facts = with_rules("\tdh $@\n", PackagingFile::Text("\n".into()));
        assert_eq!(facts.declared_level, None);
        assert_eq!(facts.level(), 1);
        assert!(facts.compat_file_open());
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

    fn commands(facts: &DebhelperFacts) -> Vec<(&str, bool)> {
        facts
            .commands()
            .iter()
            .map(|command| (command.name(), command.in_make_conditional()))
            .collect()
    }

    #[test]
    fn commands_keep_file_order_and_repeats() {
        let rules = "\
%:\n\
\tdh $@\n\
build:\n\
\t-dh_clideps\n\
\t@FOO=1 dh_lisp || true\n\
\tdh_lisp\n\
\tdh_autoreconf\n\
#\tdh_scour\n\
override_dh_auto_build:\n";
        let facts = with_rules(rules, PackagingFile::Missing);
        assert_eq!(
            commands(&facts),
            vec![
                ("dh_clideps", false),
                ("dh_lisp", false),
                ("dh_lisp", false)
            ]
        );
    }

    #[test]
    fn make_conditionals_are_counted_before_comments() {
        let rules = "\
\tdh_testroot\n\
ifeq \"$(A)\" \"y\"\n\
\tdh_python3\n\
else\n\
  ifeq \"$(B)\" \"y\"\n\
\tdh_pysupport\n\
  endif\n\
endif\n\
\tdh_scour\n\
endif\n\
\tdh_lisp\n";
        let facts = with_rules(rules, PackagingFile::Missing);
        assert_eq!(
            commands(&facts),
            vec![
                ("dh_testroot", false),
                ("dh_python3", true),
                // Indented `ifeq` and `endif` are not counted; lintian
                // anchors at column 0.
                ("dh_pysupport", true),
                ("dh_scour", false),
                // A stray `endif` leaves the counter negative, which Perl
                // also treats as skipping.
                ("dh_lisp", true),
            ]
        );
    }

    #[test]
    fn continuations_join_with_the_newline_kept() {
        // The joined line is `\tFOO=1\n dh_lisp`: `\s+` spans the newline.
        let facts = with_rules("\tFOO=1 \\\n dh_lisp\n", PackagingFile::Missing);
        assert_eq!(commands(&facts), vec![("dh_lisp", false)]);
        // A backslash before `\r\n` is no continuation in Perl.
        let facts = with_rules("\techo \\\r\n\tdh_lisp\r\n", PackagingFile::Missing);
        assert_eq!(commands(&facts), vec![("dh_lisp", false)]);
        // A comment swallows the line it continues into.
        let facts = with_rules("\tdh $@\n# \\\n\tdh_lisp\n", PackagingFile::Missing);
        assert!(facts.uses_debhelper());
        assert!(commands(&facts).is_empty());
    }

    #[test]
    fn unused_debhelper_has_no_commands() {
        let facts = with_rules("\tdh_autoreconf\n", PackagingFile::Missing);
        assert!(!facts.uses_debhelper());
        assert!(facts.commands().is_empty());
        assert!(facts.addons().is_empty());
    }

    #[test]
    fn addons_are_read_from_dh_lines() {
        let rules = "\
%:\n\
\tdh $@ --with autotools-dev,python3 --with=gnome\n\
\tdh $@ --with 'quilt' --with \"a,,b\" --with foo'bar'\n\
\tdh_lisp --with quilt\n\
ifeq \"$(A)\" \"y\"\n\
\tdh $@ --with systemd\n\
endif\n\
\tdh $@ --with 'open\n\
#\tdh $@ --with scour\n";
        let facts = with_rules(rules, PackagingFile::Missing);
        assert_eq!(
            facts.addons(),
            [
                "autotools_dev",
                "python3",
                "gnome",
                "quilt",
                "a",
                "b",
                "foo'bar'",
                "systemd",
                "'open",
            ]
        );
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
