//! Maintainer scripts as the `debian/debconf` Check scans them.
//!
//! The shebang line is consumed before the loop, and the line counter starts
//! at 1, so a hit's position is one less than its physical line. A trailing
//! `\` joins the next physical line and the hit points at that later line.
//! `#` comments are removed on the first physical line only. Files without
//! `#!` are skipped. Order is `config`, `prerm`, `postrm`, `preinst`,
//! `postinst`.

use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use super::files;
use crate::lint::file_tree::FileTree;

const SCRIPT_NAMES: &[&str] = &["config", "prerm", "postrm", "preinst", "postinst"];

static DB_CALL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?s)\A\s*(?:db_input|db_text)\s+["']?(\S+?)["']?\s+(\S+)\s"#)
        .expect("valid regex")
});
static SHELL_PRIORITY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\$\S+$").expect("valid regex"));
static OBSOLETE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"/usr/share/debconf/confmodule\.sh|Debian::DebConf::Client::ConfModule")
        .expect("valid regex")
});

pub(super) struct ScannedScript {
    pub path: PathBuf,
    pub lines: Vec<ScriptLine>,
}

pub(super) struct ScriptLine {
    pub position: u32,
    pub text: String,
}

pub(super) struct DbInvocation {
    pub priority: String,
    pub template: String,
}

impl ScannedScript {
    /// The processed line at `position` contains `/dev/`. Blank lines are not
    /// processed, so they do not count.
    pub(super) fn mentions_dev(&self, position: u32) -> bool {
        self.lines
            .iter()
            .any(|line| line.position == position && line.text.contains("/dev/"))
    }
}

pub(super) fn scan(files: &FileTree) -> Vec<ScannedScript> {
    let mut scripts = Vec::new();
    for name in SCRIPT_NAMES {
        let Some(file) = files::read_lossy(files, name) else {
            continue;
        };
        let Some(lines) = logical_lines(&file.text) else {
            continue;
        };
        scripts.push(ScannedScript {
            path: file.path,
            lines,
        });
    }
    scripts
}

pub(super) fn db_invocation(text: &str) -> Option<DbInvocation> {
    let captures = DB_CALL.captures(text)?;
    Some(DbInvocation {
        priority: captures[1].to_string(),
        template: captures[2].to_string(),
    })
}

pub(super) fn shell_priority(priority: &str) -> bool {
    SHELL_PRIORITY.is_match(priority)
}

pub(super) fn valid_priority(priority: &str) -> bool {
    matches!(priority, "low" | "medium" | "high" | "critical")
}

pub(super) fn obsolete_module(text: &str) -> Option<String> {
    OBSOLETE.find(text).map(|found| found.as_str().to_string())
}

struct Phys {
    content: String,
    newline: bool,
}

/// `None` when the first line is not a shebang.
fn logical_lines(text: &str) -> Option<Vec<ScriptLine>> {
    let lines = physical(text);
    let first = lines.first()?;
    if !first.content.starts_with("#!") {
        return None;
    }
    let mut logical = Vec::new();
    let mut position = 1u32;
    let mut index = 1usize;
    while index < lines.len() {
        let mut body = strip_comment(&lines[index].content);
        let mut newline = lines[index].newline;
        index += 1;
        if !body.chars().any(|ch| !ch.is_whitespace()) {
            position += 1;
            continue;
        }
        while body.ends_with('\\') && newline {
            body.pop();
            if index >= lines.len() {
                break;
            }
            position += 1;
            body.push('\n');
            body.push_str(&lines[index].content);
            newline = lines[index].newline;
            index += 1;
        }
        if newline {
            body.push('\n');
        }
        logical.push(ScriptLine {
            position,
            text: body,
        });
        position += 1;
    }
    Some(logical)
}

fn strip_comment(line: &str) -> String {
    match line.find('#') {
        Some(hash) => line[..hash].to_string(),
        None => line.to_string(),
    }
}

fn physical(text: &str) -> Vec<Phys> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut lines = Vec::new();
    let mut rest = text;
    loop {
        if rest.is_empty() {
            break;
        }
        if let Some(index) = rest.find('\n') {
            lines.push(Phys {
                content: rest[..index].to_string(),
                newline: true,
            });
            rest = &rest[index + 1..];
            if rest.is_empty() {
                break;
            }
        } else {
            lines.push(Phys {
                content: rest.to_string(),
                newline: false,
            });
            break;
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_follow_the_shebang_pread() {
        let text = "\
#!/bin/sh

set -e

# Forget to load the confmodule
# . /usr/share/debconf/confmodule


db_input low debconf/note-abuse
db_go


db_input unknown debconf/unknown-priority
";
        let lines = logical_lines(text).expect("shebang");
        let abuse = lines
            .iter()
            .find(|line| line.text.contains("note-abuse"))
            .expect("abuse");
        assert_eq!(abuse.position, 8);
        let unknown = lines
            .iter()
            .find(|line| line.text.contains("unknown"))
            .expect("unknown");
        assert_eq!(unknown.position, 12);
    }

    #[test]
    fn continuation_points_at_the_later_line() {
        let text = "#!/bin/sh\ndb_input \\\nunknown foo\n";
        let lines = logical_lines(text).expect("shebang");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].position, 2);
        let call = db_invocation(&lines[0].text).expect("call");
        assert_eq!(call.priority, "unknown");
        assert_eq!(call.template, "foo");
    }

    #[test]
    fn quoted_and_variable_priorities() {
        let medium = db_invocation("db_input 'medium' debconf/test\n").expect("quoted");
        assert_eq!(medium.priority, "medium");
        assert!(!shell_priority(&medium.priority));
        let variable = db_invocation("db_input \"$foo\" debconf/test\n").expect("variable");
        assert_eq!(variable.priority, "$foo");
        assert!(shell_priority(&variable.priority));
        assert!(valid_priority("high"));
        assert!(!valid_priority("LOW"));
        assert!(!valid_priority("normal"));
    }

    #[test]
    fn no_shebang_is_skipped() {
        assert!(logical_lines(". /usr/share/debconf/confmodule.sh\n").is_none());
    }
}
