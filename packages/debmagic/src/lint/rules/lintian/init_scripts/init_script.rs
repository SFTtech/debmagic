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

pub(super) struct InitDScript {
    pub path: PathBuf,
    pub missing_commands: Vec<&'static str>,
    pub has_lsb: bool,
    pub stops_in_s: usize,
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
            Some(InitDScript {
                path: entry.path().to_path_buf(),
                missing_commands: missing_commands(&text),
                has_lsb: has_lsb_section(&text),
                stops_in_s: stops_in_s(&text),
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

fn stops_in_s(text: &str) -> usize {
    default_stop(text)
        .into_iter()
        .filter(|level| level == "S" || level == "s")
        .count()
}

/// Runlevels from the last `Default-Stop` in the first LSB section.
fn default_stop(text: &str) -> Vec<String> {
    let mut lines = text.lines();
    let mut levels = Vec::new();
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
            if caps[1].eq_ignore_ascii_case("default-stop") {
                levels = caps
                    .get(2)
                    .map(|value| value.as_str())
                    .unwrap_or("")
                    .split_whitespace()
                    .map(str::to_string)
                    .collect();
            }
        }
        break;
    }
    levels
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
}
