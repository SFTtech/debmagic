//! `etc/init.d` script text, as the `init.d` and `systemd` checks read it.
//!
//! Scripts are regular files and hardlinks. Symlinks have no bytes here, so
//! they are not scanned (an upstart-job link is skipped by both checks).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::file_tree::{Entry, EntryKind, FileTree};

const REQUIRED: [&str; 4] = ["start", "stop", "restart", "force-reload"];
const SPECIAL: &[&str] = &["README", "skeleton", "rc", "rcS"];

static ENV_PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^/usr/bin/env\s+").expect("valid regex"));
static SOURCES_INIT_D_SCRIPT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\. /lib/init/init-d-script").expect("valid regex"));
/// Literal alternation, so a longer word wins at the same position (`status`
/// is not `start`). That is Perl's trie, which this check uses.
static COMMAND: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[^#]*?(start|stop|restart|force-reload|status)").expect("valid regex")
});
static LSB_KEYWORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^# ([A-Za-z-]+):\s*(.*?)\s*$").expect("valid regex"));
static RCS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"#.*Default-Start:.*S").expect("valid regex"));
static EXCLUDE_UPDATE_RC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"if\s+\[\s+-x\s+\S*update-rc\.d").expect("valid regex"));
static UPDATE_RC: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?:.+;|^\s*system[\s(']+)?\s*update-rc\.d\s+(?:-\S+\s*)*([A-Za-z0-9_.+][A-Za-z0-9_.+-]*)\s+([A-Za-z0-9_]+)",
    )
    .expect("valid regex")
});

static WHITELIST: LazyLock<Vec<String>> = LazyLock::new(|| {
    crate::lint::data::lines(include_str!("../../../data/systemd/init-whitelist"))
        .map(str::to_string)
        .collect()
});

/// Facilities from the first LSB section. The last assignment of each keyword
/// wins. A second `### BEGIN INIT INFO` is ignored.
#[derive(Debug, Default)]
pub(super) struct LsbHeader {
    pub default_start: Vec<String>,
    pub default_stop: Vec<String>,
    pub required_start: Vec<String>,
    pub should_start: Vec<String>,
    pub required_stop: Vec<String>,
    pub should_stop: Vec<String>,
}

pub(super) struct InitDScript {
    pub path: PathBuf,
    pub missing_commands: Vec<&'static str>,
    pub has_lsb: bool,
    pub stops_in_s: usize,
    pub lsb: LsbHeader,
}

/// A matching line in a direct child of `etc/default`.
pub(super) struct DefaultEnableHit {
    pub path: PathBuf,
    pub line: u32,
}

pub(super) struct RcSScript {
    pub path: PathBuf,
    pub service: String,
}

pub(super) fn init_d_scripts(
    data: &FileTree,
    registered: &HashSet<String>,
    package: &str,
) -> Vec<InitDScript> {
    if matches!(package, "initscripts" | "sysvinit") {
        return Vec::new();
    }
    script_entries(data)
        .filter_map(|entry| {
            let base = entry.path().file_name()?.to_str()?;
            if SPECIAL.contains(&base) && !registered.contains(base) {
                return None;
            }
            let text = read_text(data, entry)?;
            let lsb = lsb_header(&text);
            Some(InitDScript {
                path: entry.path().to_path_buf(),
                missing_commands: missing_commands(&text),
                has_lsb: has_lsb_section(&text),
                stops_in_s: lsb
                    .default_stop
                    .iter()
                    .filter(|level| *level == "S" || *level == "s")
                    .count(),
                lsb,
            })
        })
        .collect()
}

/// rcS init scripts the systemd check would consider, before the whitelist
/// and "name already provided by a unit" filters.
pub(super) fn rc_s_scripts(data: &FileTree, package: &str) -> Vec<RcSScript> {
    if package == "initscripts" {
        return Vec::new();
    }
    script_entries(data)
        .filter_map(|entry| {
            let base = entry.path().file_name()?.to_str()?;
            if SPECIAL.contains(&base) {
                return None;
            }
            let text = read_text(data, entry)?;
            if !is_rc_s(&text) {
                return None;
            }
            let service = base.strip_suffix(".sh").unwrap_or(base).to_string();
            Some(RcSScript {
                path: entry.path().to_path_buf(),
                service,
            })
        })
        .collect()
}

pub(super) fn whitelisted(service: &str) -> bool {
    WHITELIST.iter().any(|name| name == service)
}

fn script_entries(data: &FileTree) -> impl Iterator<Item = &Entry> {
    data.entries().iter().filter(|entry| {
        entry.path().parent() == Some(Path::new("etc/init.d"))
            && matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink)
    })
}

fn read_text(files: &FileTree, entry: &Entry) -> Option<String> {
    files
        .read(entry)
        .ok()
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
}

fn missing_commands(text: &str) -> Vec<&'static str> {
    let mut saw = [false; 4];
    if init_d_script_interpreter(text) {
        saw = [true; 4];
    }
    for line in text.lines() {
        if SOURCES_INIT_D_SCRIPT.is_match(line) {
            saw = [true; 4];
        }
        let mut rest = line;
        loop {
            let found = COMMAND.captures(rest).and_then(|caps| {
                caps.get(1)
                    .map(|word| (word.as_str().to_string(), word.end()))
            });
            let Some((token, end)) = found else {
                break;
            };
            mark(&mut saw, &token);
            if end == 0 || end >= rest.len() {
                break;
            }
            rest = &rest[end..];
        }
    }
    REQUIRED
        .into_iter()
        .enumerate()
        .filter(|(index, _)| !saw[*index])
        .map(|(_, command)| command)
        .collect()
}

fn mark(saw: &mut [bool; 4], token: &str) {
    let index = match token {
        "start" => 0,
        "stop" => 1,
        "restart" => 2,
        "force-reload" => 3,
        _ => return,
    };
    saw[index] = true;
}

/// Shebang interpreter, `env` stripped, without an interpreter table.
fn init_d_script_interpreter(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.starts_with(b"#!#!#!") || !bytes.starts_with(b"#!") || bytes.len() <= 2 {
        return false;
    }
    let line = text[2..].lines().next().unwrap_or("");
    let line = ENV_PREFIX.replace(line.trim(), "");
    line.split_whitespace().next() == Some("/lib/init/init-d-script")
}

fn has_lsb_section(text: &str) -> bool {
    text.lines()
        .any(|line| line.starts_with("### BEGIN INIT INFO"))
}

/// Known virtual facilities (`data/init.d/virtual_facilities`). The
/// unknown-facility Tag consults this list; `$all` itself is matched literally.
#[allow(dead_code)]
pub(super) const VIRTUAL_FACILITIES: &[&str] = &[
    "$all",
    "$local_fs",
    "$mail-transport-agent",
    "$named",
    "$network",
    "$portmap",
    "$remote_fs",
    "$syslog",
    "$time",
    "$x-display-manager",
];

/// Non-virtual facility names and the virtual facility that should replace them.
/// `checkroot` is intentionally absent.
const IMPLIED_FACILITIES: &[(&str, &str)] = &[
    ("mountall", "$local_fs"),
    ("mountnfs", "$remote_fs"),
    ("hwclock", "$time"),
    ("portmap", "$portmap"),
    ("named", "$named"),
    ("bind9", "$named"),
    ("networking", "$network"),
    ("syslog", "$syslog"),
    ("rsyslog", "$syslog"),
    ("sysklogd", "$syslog"),
];

/// `etc/default` lines that hide a service from the init system. `/x` drops the
/// spaces in lintian's pattern, and `#` is not a verbose-mode comment here.
static ALWAYS_START: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*#*\s*(?:[A-Z]_)?(?:ENABLED|DISABLED|[A-Z]*RUN|(?:NO_)?START)=")
        .expect("valid regex")
});

/// Keywords of the first LSB section. A duplicate keyword overwrites. The
/// section ends at `### END INIT INFO` or the first line that is not a comment.
fn lsb_header(text: &str) -> LsbHeader {
    let mut header = LsbHeader::default();
    let mut lines = text.lines();
    for line in lines.by_ref() {
        if !line.starts_with("### BEGIN INIT INFO") {
            continue;
        }
        for inner in lines.by_ref() {
            if inner.starts_with("### END INIT INFO") || !inner.starts_with('#') {
                break;
            }
            let Some(caps) = LSB_KEYWORD.captures(inner) else {
                continue;
            };
            let parts = caps
                .get(2)
                .map(|value| value.as_str())
                .unwrap_or("")
                .split_whitespace()
                .map(str::to_string)
                .collect();
            match caps[1].to_ascii_lowercase().as_str() {
                "default-start" => header.default_start = parts,
                "default-stop" => header.default_stop = parts,
                "required-start" => header.required_start = parts,
                "should-start" => header.should_start = parts,
                "required-stop" => header.required_stop = parts,
                "should-stop" => header.should_stop = parts,
                _ => {}
            }
        }
        break;
    }
    header
}

impl LsbHeader {
    pub(super) fn facilities(&self, keyword: &str) -> &[String] {
        match keyword {
            "required-start" => &self.required_start,
            "should-start" => &self.should_start,
            "required-stop" => &self.required_stop,
            "should-stop" => &self.should_stop,
            _ => &[],
        }
    }
}

pub(super) fn implied_facility(name: &str) -> Option<&'static str> {
    IMPLIED_FACILITIES
        .iter()
        .find(|(facility, _)| *facility == name)
        .map(|(_, replacement)| *replacement)
}

/// One character from `s`, `S`, or `0`–`6`.
pub(super) fn valid_runlevel(level: &str) -> bool {
    matches!(level.as_bytes(), [b's' | b'S' | b'0'..=b'6'])
}

/// Valid `Default-Start` tokens, lowercased. `S` and `s` share the key `s`.
pub(super) fn start_keys(levels: &[String]) -> HashSet<String> {
    levels
        .iter()
        .filter(|level| valid_runlevel(level))
        .map(|level| level.to_ascii_lowercase())
        .collect()
}

/// Direct regular files and hardlinks under `etc/default`. A directory child
/// does not stop the scan: lintian's child order is a hash, so aborting on the
/// first unreadable child is not reproducible.
pub(super) fn default_enable_lines(data: &FileTree, package: &str) -> Vec<DefaultEnableHit> {
    if matches!(package, "initscripts" | "sysvinit") {
        return Vec::new();
    }
    let mut hits = Vec::new();
    for entry in data.entries() {
        if entry.path().parent() != Some(Path::new("etc/default")) {
            continue;
        }
        if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
            continue;
        }
        let Some(text) = read_text(data, entry) else {
            continue;
        };
        for (index, line) in text.lines().enumerate() {
            if ALWAYS_START.is_match(line) {
                hits.push(DefaultEnableHit {
                    path: entry.path().to_path_buf(),
                    line: (index as u32) + 1,
                });
            }
        }
    }
    hits
}

fn is_rc_s(text: &str) -> bool {
    text.lines().any(|line| RCS.is_match(line.trim_start()))
}

pub(super) fn registered_names(control: &FileTree) -> HashSet<String> {
    let Some(entry) = control.get("postinst") else {
        return HashSet::new();
    };
    if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
        return HashSet::new();
    }
    let Some(text) = read_text(control, entry) else {
        return HashSet::new();
    };
    let mut names = HashSet::new();
    for line in text.lines() {
        if EXCLUDE_UPDATE_RC.is_match(line) {
            continue;
        }
        let code = match line.find('#') {
            Some(index) => &line[..index],
            None => line,
        };
        let Some(caps) = UPDATE_RC.captures(code) else {
            continue;
        };
        if &caps[2] == "remove" {
            continue;
        }
        names.insert(caps[1].to_string());
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    fn postinst(text: &str) -> FileTree {
        FileTree::from_entries(
            None,
            vec![Entry::regular_file("postinst", None, Some(text.into()))],
        )
    }

    #[test]
    fn postinst_registers_update_rc_names() {
        let tree = postinst(
            "#!/bin/sh\n\
             if [ -x \"/etc/init.d/lsb-broken\" ] ; then\n\
                 update-rc.d lsb-broken defaults >/dev/null\n\
             fi\n\
             update-rc.d skeleton remove\n\
             update-rc.d -f kept defaults\n\
             # update-rc.d hidden defaults\n\
             if [ -x update-rc.d ]; then update-rc.d skipped defaults; fi\n",
        );
        let names = registered_names(&tree);
        assert!(names.contains("lsb-broken"));
        assert!(names.contains("kept"));
        assert!(!names.contains("skeleton"));
        assert!(!names.contains("hidden"));
        assert!(!names.contains("skipped"), "{names:?}");
    }

    #[test]
    fn status_word_is_not_start_and_restart_is_not_start() {
        assert_eq!(
            missing_commands("case \"$1\" in\n  status) ;;\nesac\n"),
            REQUIRED
        );
        assert_eq!(
            missing_commands("restart\n"),
            ["start", "stop", "force-reload"]
        );
        assert!(missing_commands("do_start\ndo_stop\nrestart\nforce-reload\n").is_empty());
    }

    #[test]
    fn comment_hides_options_and_init_d_script_provides_them() {
        assert_eq!(
            missing_commands("# start stop restart force-reload\nexit 0\n"),
            REQUIRED
        );
        assert!(missing_commands("#!/lib/init/init-d-script\nexit 0\n").is_empty());
        assert!(missing_commands("#!/usr/bin/env /lib/init/init-d-script\nexit 0\n").is_empty());
        assert!(missing_commands("#!/bin/sh\n. /lib/init/init-d-script\n").is_empty());
    }

    #[test]
    fn virtual_facilities_match_the_lintian_table() {
        assert!(VIRTUAL_FACILITIES.contains(&"$all"));
        assert!(VIRTUAL_FACILITIES.contains(&"$local_fs"));
        assert!(!VIRTUAL_FACILITIES.contains(&"$something_that_doesnt_exist"));
        assert_eq!(VIRTUAL_FACILITIES.len(), 10);
        assert_eq!(implied_facility("bind9"), Some("$named"));
        assert_eq!(implied_facility("checkroot"), None);
    }

    #[test]
    fn lsb_header_keeps_the_last_keyword_in_the_first_section() {
        let text = "\
### BEGIN INIT INFO
# Default-Stop: 0 1 6
# Default-Stop: X
# Required-Start: $all
### END INIT INFO
### BEGIN INIT INFO
# Default-Start: 0
# Required-Start: networking
### END INIT INFO
";
        let header = lsb_header(text);
        assert_eq!(header.default_stop, ["X"]);
        assert_eq!(header.required_start, ["$all"]);
        assert!(header.default_start.is_empty());
    }

    #[test]
    fn always_start_matches_the_init_d_pattern() {
        assert!(ALWAYS_START.is_match("ENABLED=false"));
        assert!(ALWAYS_START.is_match("# ENABLED=false"));
        assert!(ALWAYS_START.is_match("##ENABLED="));
        assert!(ALWAYS_START.is_match("  PREFIXRUN=true"));
        assert!(ALWAYS_START.is_match("A_NO_START=1"));
        assert!(ALWAYS_START.is_match("NO_START=1"));
        assert!(ALWAYS_START.is_match("\tDISABLED=1"));
        assert!(!ALWAYS_START.is_match("enabled="));
        assert!(!ALWAYS_START.is_match("ENABLED =1"));
        assert!(!ALWAYS_START.is_match("AB_ENABLED=1"));
        assert!(!ALWAYS_START.is_match("# # ENABLED=1"));
    }
}
