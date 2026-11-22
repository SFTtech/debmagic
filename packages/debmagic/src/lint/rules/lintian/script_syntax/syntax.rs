//! lintian's `Check::Script::Syntax`: a script whose shell rejects it with
//! `-n`. Shared by the Rules for maintainer scripts and for installed scripts.
//!
//! Only `sh` and `bash` interpreters are checked (`/bin/dash -n` and
//! `/bin/bash -n`; zsh is left out as lintian leaves it out). A file that
//! looks like a re-exec wrapper rather than a script is not checked.

use std::path::{Component, Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::external_tool;
use crate::lint::file_tree::{Entry, FileTree};
use crate::lint::interpreter::{Hashbang, basename};

/// Interpreter basename and the program that checks its syntax.
const CHECKERS: [(&str, &str); 2] = [("sh", "/bin/dash"), ("bash", "/bin/bash")];

/// lintian's `$MAXIMUM_LINES_ANALYZED`: lines from the start of the file that
/// the wrapper test reads.
const MAXIMUM_LINES_ANALYZED: usize = 54;

/// Symlinks followed before giving up on a path.
const MAXIMUM_LINKS: usize = 16;

/// A checker found on this host: the interpreter basename it serves and the
/// program to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Checker {
    pub(super) shell: &'static str,
    pub(super) program: PathBuf,
}

/// The checkers that exist on this host. A missing one means the Rule has no
/// verdict for that interpreter, as lintian's `-x` test makes it.
pub(super) fn host_checkers() -> Vec<Checker> {
    CHECKERS
        .iter()
        .filter_map(|(shell, program)| {
            Some(Checker {
                shell,
                program: external_tool::find(program)?,
            })
        })
        .collect()
}

/// Whether `entry` (a file, or a symlink to one in the tree) is a script that
/// its interpreter rejects with `-n`.
///
/// False when there is no checker for the interpreter, when the file is not
/// openable, when it looks like a wrapper, or when the check cannot run.
pub(super) fn fails_syntax_check(files: &FileTree, entry: &Entry, checkers: &[Checker]) -> bool {
    let Some(target) = open_target(files, entry) else {
        return false;
    };
    let Ok(bytes) = files.read(target) else {
        return false;
    };
    let Some(hashbang) = Hashbang::read(&bytes) else {
        return false;
    };
    let interpreter = hashbang.interpreter();
    if interpreter.is_empty() {
        return false;
    }
    let name = basename(&interpreter);
    let Some(checker) = checkers.iter().find(|checker| checker.shell == name) else {
        return false;
    };
    if looks_dangerous(&bytes) {
        return false;
    }
    let args = [std::ffi::OsStr::new("-n"), target.path().as_os_str()];
    external_tool::run_in_tree(
        files,
        &[target],
        &checker.program,
        &args,
        external_tool::DEFAULT_TIMEOUT,
    )
    .is_ok_and(|captured| !captured.success())
}

/// lintian's `is_open_ok`: the Entry itself when it is a regular file or a
/// hardlink, or the file a symlink resolves to inside the tree.
fn open_target<'a>(files: &'a FileTree, entry: &'a Entry) -> Option<&'a Entry> {
    let mut current = entry;
    for _ in 0..MAXIMUM_LINKS {
        match current {
            Entry::RegularFile { .. } | Entry::Hardlink { .. } => return Some(current),
            Entry::Symlink { .. } => {
                let target = current.link_target()?;
                let parent = current.path().parent().unwrap_or(Path::new(""));
                current = files.get(normalise(&parent.join(target))?)?;
            }
            _ => return None,
        }
    }
    None
}

/// Resolves `.` and `..` lexically. `None` when the path climbs above the
/// tree root.
fn normalise(path: &Path) -> Option<PathBuf> {
    let mut parts = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(name) => parts.push(name),
            Component::ParentDir => {
                if !parts.pop() {
                    return None;
                }
            }
            Component::CurDir | Component::RootDir | Component::Prefix(_) => {}
        }
    }
    Some(parts)
}

/// lintian's `script_looks_dangerous`: the script re-execs itself or runs a
/// backgrounded copy of itself, so it is not a plain shell script and is not
/// syntax-checked.
pub(super) fn looks_dangerous(content: &[u8]) -> bool {
    let text = String::from_utf8_lossy(content);
    let mut variable = "0".to_string();
    let mut patterns = wrapper_patterns(&variable);
    let mut backgrounded = false;

    for (index, line) in text.split('\n').enumerate() {
        // lintian's `$position` counts every line from 1.
        let position = index + 1;
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if position >= MAXIMUM_LINES_ANALYZED {
            break;
        }
        if patterns.exec.is_match(line) {
            return true;
        }
        if let Some(captures) = SHELL_VARIABLE.captures(line) {
            variable = captures[1].to_string();
            patterns = wrapper_patterns(&variable);
        } else if patterns.background.is_match(line) {
            backgrounded = true;
        } else if backgrounded && EXEC_TRUE.is_match(line) {
            return true;
        }
    }
    false
}

struct WrapperPatterns {
    exec: Regex,
    background: Regex,
}

/// The two patterns that depend on the variable holding `$0`.
fn wrapper_patterns(variable: &str) -> WrapperPatterns {
    let dollar_name = format!(r"\${variable}");
    let exec = format!(
        r#"(?x)
        (?:^\s*|\beval\s*['"]|(?:;|&&|\b(?:then|else))\s*)
        exec\s*.+\s*
        .?{dollar_name}.?\s*
        (?:--\s*)?
        .?(?:\$[{{]1:?\+.?)?(?:\$[@*])?
        "#
    );
    let background = format!(
        r"(?x)
        \S+\s+
        .?{dollar_name}.?\s*
        (?:--\s*)?
        .?(?:\$[{{]1:?\+.?)?(?:\$[@*])?.?\s*&
        "
    );
    WrapperPatterns {
        exec: Regex::new(&exec).expect("valid regex"),
        background: Regex::new(&background).expect("valid regex"),
    }
}

static SHELL_VARIABLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(\w+)=\$0;").expect("valid regex"));

static EXEC_TRUE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?:^\s*|\beval\s*['"]|(?:;|&&|\b(?:then|else))\s*)exec\s+true(?:\s|$)"#)
        .expect("valid regex")
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrappers_are_detected() {
        // lintian's `exec "$0" "$@"` re-exec of the same script.
        assert!(looks_dangerous(b"#!/bin/sh\nexec \"$0\" \"$@\"\n"));
        // The `${1+"$@"}` form of the same re-exec.
        assert!(looks_dangerous(b"#!/bin/sh\nexec \"$0\" ${1+\"$@\"}\n"));
        // A variable copied from $0 is followed.
        assert!(looks_dangerous(b"#!/bin/sh\nme=$0;\nexec \"$me\" \"$@\"\n"));
        // A backgrounded copy of itself, then `exec true`.
        assert!(looks_dangerous(
            b"#!/bin/sh\n/usr/bin/foo $0 $@ &\nexec true\n"
        ));
    }

    #[test]
    fn plain_scripts_are_not_wrappers() {
        assert!(!looks_dangerous(b"#!/bin/sh\necho hello\n"));
        // `exec` of another program is not a re-exec of $0.
        assert!(!looks_dangerous(b"#!/bin/sh\nexec /usr/bin/tool \"$@\"\n"));
        // A backgrounded program that is not $0, then `exec true`.
        assert!(!looks_dangerous(b"#!/bin/sh\nsleep 1 &\nexec true\n"));
        // Wrapper lines past line 54 are not read.
        let mut late = String::from("#!/bin/sh\n");
        for _ in 0..60 {
            late.push_str("echo x\n");
        }
        late.push_str("exec \"$0\" \"$@\"\n");
        assert!(!looks_dangerous(late.as_bytes()));
    }

    #[test]
    fn normalise_resolves_dots_and_refuses_escape() {
        assert_eq!(
            normalise(Path::new("usr/bin/./../lib/x")),
            Some(PathBuf::from("usr/lib/x"))
        );
        assert_eq!(normalise(Path::new("../etc/x")), None);
    }
}
