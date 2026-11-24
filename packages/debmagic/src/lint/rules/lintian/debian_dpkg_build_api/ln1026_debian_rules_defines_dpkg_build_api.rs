use std::collections::VecDeque;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::FileTree;
use crate::lint::rule::SourceRule;

pub struct DebianRulesDefinesDpkgBuildApi;

declare_rule! {
    /// `debian/rules` assigns `DPKG_BUILD_API`.
    ///
    /// The file is read through symlinks, as lintian does, and must be a
    /// regular file or hardlink in the tree. Lines are joined over trailing
    /// backslashes. Comment lines are skipped. Assignments inside an `ifeq`,
    /// `ifneq`, `ifdef` or `ifndef` block are skipped, and so is everything
    /// after an `endif` without its opener, as lintian does. The first
    /// `DPKG_BUILD_API =`, `:=`, `::=`, `?=`, `+=`, `!=`, `export`/`override`
    /// form, or `define DPKG_BUILD_API`, is reported. Extra is empty. The
    /// Location is `debian/rules` at the last physical line of that logical
    /// line. The Rule checks the Source tree and Source package.
    DebianRulesDefinesDpkgBuildApi,
    code = "LN1026",
    tag = "debian-rules-defines-dpkg-build-api",
    default_selected = true,
    experimental = false,
}

static CONDITIONAL_OPEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*ifn?(?:eq|def)\b").expect("valid regex"));
static CONDITIONAL_CLOSE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*endif\b").expect("valid regex"));
static COMMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*#").expect("valid regex"));
static ASSIGNMENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^\s*(?:export\s+|override\s+)?DPKG_BUILD_API\s*(?:=|:=|::=|\?=|\+=|!=)|^\s*(?:export\s+|override\s+)?define\s+DPKG_BUILD_API",
    )
    .expect("valid regex")
});

impl SourceRule for DebianRulesDefinesDpkgBuildApi {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let found = {
            let Some(files) = ctx.files() else {
                return;
            };
            let Some(path) = resolve(files, Path::new("debian/rules")) else {
                return;
            };
            let Some(entry) = files.get(&path) else {
                return;
            };
            if !entry.is_file() {
                return;
            }
            let Ok(bytes) = files.read(entry) else {
                return;
            };
            assignment_line(&bytes).map(|line| (path, line))
        };
        if let Some((path, line)) = found {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
                line: Some(line),
                column: None,
            }));
        }
    }
}

/// Physical line of the first assignment that counts, or `None`. Lines are
/// read the way lintian's loop reads them: a backslash at the end of a line
/// joins the next one, and the reported number is the last of those lines.
fn assignment_line(bytes: &[u8]) -> Option<u32> {
    let text = String::from_utf8_lossy(bytes);
    let mut lines = text.split_inclusive('\n');
    let mut numline = 0u32;
    // Perl's undefined counter reads as zero, and a negative one still skips.
    let mut skipping = 0i64;
    while let Some(first) = lines.next() {
        numline += 1;
        let mut line = first.to_string();
        loop {
            let body = line.strip_suffix('\n').unwrap_or(&line);
            let Some(head) = body.strip_suffix('\\') else {
                break;
            };
            let mut joined = head.to_string();
            joined.push_str(&line[body.len()..]);
            match lines.next() {
                Some(next) => {
                    numline += 1;
                    joined.push_str(next);
                    line = joined;
                }
                None => {
                    line = joined;
                    break;
                }
            }
        }
        if COMMENT.is_match(&line) {
            continue;
        }
        if CONDITIONAL_OPEN.is_match(&line) {
            skipping += 1;
        } else if CONDITIONAL_CLOSE.is_match(&line) {
            skipping -= 1;
        }
        if skipping != 0 {
            continue;
        }
        if ASSIGNMENT.is_match(&line) {
            return Some(numline);
        }
    }
    None
}

/// Lintian's `resolve_path`: walks `path` and follows every symlink on the
/// way, including a final one. `None` when a link leaves the tree, is
/// absolute, loops, or dangles.
fn resolve(tree: &FileTree, path: &Path) -> Option<PathBuf> {
    const MAXIMUM_LINKS: usize = 8;
    let mut resolved = PathBuf::new();
    let mut pending: VecDeque<OsString> = path.iter().map(|part| part.to_os_string()).collect();
    let mut links = 0usize;
    while let Some(part) = pending.pop_front() {
        match part.as_os_str().to_str() {
            Some(".") => continue,
            Some("..") => {
                if !resolved.pop() {
                    return None;
                }
                continue;
            }
            _ => {}
        }
        resolved.push(&part);
        // Only links are followed. A missing component is left for the caller.
        let Some(entry) = tree.get(&resolved).filter(|entry| entry.is_symlink()) else {
            continue;
        };
        links += 1;
        if links > MAXIMUM_LINKS {
            return None;
        }
        let target = entry.link_target()?;
        if target.is_absolute() {
            return None;
        }
        resolved.pop();
        for piece in target.iter().rev() {
            pending.push_front(piece.to_os_string());
        }
    }
    Some(resolved)
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn rules(body: &str) -> Vec<(&'static str, TestFile)> {
        vec![("debian/rules", TestFile::new(body))]
    }

    #[test]
    fn source() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            rules("#!/usr/bin/make -f\n%:\n\tdh $@\n"),
            // Commented out, and a different variable.
            rules("# DPKG_BUILD_API = 1\nDPKG_BUILD_API_X = 1\nFOO_DPKG_BUILD_API = 1\n"),
            // Inside a conditional, and after an unmatched endif.
            rules("ifeq (a,b)\nDPKG_BUILD_API = 1\nendif\n"),
            rules("ifdef X\nifdef Y\nDPKG_BUILD_API = 1\nendif\nendif\n"),
            rules("endif\nDPKG_BUILD_API = 1\n"),
            // Not an assignment: a reference or a rule line.
            rules("all:\n\techo $(DPKG_BUILD_API)\n"),
            // A symlinked `debian/rules` with no such entry is skipped.
            vec![("debian/rules", TestFile::symlink("missing"))],
        ];
        let fail = vec![
            rules("DPKG_BUILD_API = 100500\n"),
            rules("export DPKG_BUILD_API := 3\n"),
            rules("override DPKG_BUILD_API ?= 3\n"),
            rules("define DPKG_BUILD_API\n3\nendef\n"),
            // The value is on the last physical line of a continued line.
            rules("X = 1\nDPKG_BUILD_API \\\n  = 3\n"),
            // The conditional is closed before the assignment.
            rules("ifeq (a,a)\nendif\nDPKG_BUILD_API = 2\n"),
            // Symlinked `debian/rules` is followed; a hardlink is a file too.
            vec![
                ("debian/rules", TestFile::symlink("rules.real")),
                ("debian/rules.real", TestFile::new("DPKG_BUILD_API = 2\n")),
            ],
            vec![(
                "debian/rules",
                TestFile::new("DPKG_BUILD_API = 2\n").hardlink(),
            )],
        ];
        SourceTreeTester::new(DebianRulesDefinesDpkgBuildApi, pass, fail).test_and_snapshot();
    }
}
