use std::path::{Component, Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, FileTree};
use crate::lint::rule::{Severity, SourceRule};

pub struct DhInstallInsteadOfDhInstallmodules;

declare_rule! {
    /// A `debian/*.install` file installs a `modprobe.d` file by hand.
    ///
    /// Reported as an info hint, once per matching line, with the line number
    /// as the Location line and no Extra. Only the Patched `debian/` folder is
    /// checked (`debian/` directly, not below it): a file whose basename is
    /// `install` or ends in `.install`, that is a regular file, a hardlink, or
    /// a symlink that resolves inside the tree. A line matches when it starts
    /// with `debian/`, then has a blank before a final path component that
    /// ends in `/modprobe.d` (with at most one character after it). Files that
    /// are not valid UTF-8 yield no lines. Entries under `.pc/` are skipped.
    /// Both Source tree and Source package are checked.
    DhInstallInsteadOfDhInstallmodules,
    code = "LN1578",
    tag = "dh-install-instead-of-dh-installmodules",
    default_selected = false,
    experimental = false,
}

impl SourceRule for DhInstallInsteadOfDhInstallmodules {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let mut hits = Vec::new();
        for entry in files.entries() {
            if !is_install_file(entry) || !is_open_ok(files, entry) {
                continue;
            }
            let Ok(bytes) = files.read(entry) else {
                continue;
            };
            let Ok(text) = std::str::from_utf8(&bytes) else {
                continue;
            };
            for (index, line) in text.split('\n').enumerate() {
                if MODPROBE_LINE.is_match(line) {
                    let line_no = u32::try_from(index + 1).unwrap_or(u32::MAX);
                    hits.push((entry.path().to_path_buf(), line_no));
                }
            }
        }
        for (path, line) in hits {
            ctx.diagnostic(
                Diagnostic::warn("")
                    .with_severity(Severity::Info)
                    .with_location(Location {
                        path,
                        line: Some(line),
                        column: None,
                    }),
            );
        }
    }
}

/// lintian's `^debian\/.+[[:blank:]]+.+\/modprobe\.d.?$`.
static MODPROBE_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^debian/.+[[:blank:]]+.+/modprobe\.d.?$").expect("valid regex"));

/// `visit_patched_files` for `debian/` items: the parent is `debian` and the
/// name is `install` or ends in `.install`. Entries under `.pc/` are skipped.
fn is_install_file(entry: &Entry) -> bool {
    let path = entry.path();
    if path.starts_with(".pc") {
        return false;
    }
    if path.parent() != Some(Path::new("debian")) {
        return false;
    }
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    name == "install" || name.ends_with(".install")
}

/// lintian's `is_open_ok`: a regular file or hardlink, or a symlink that
/// resolves to one inside the tree. Symlinks are followed up to a fixed depth.
fn is_open_ok(files: &FileTree, entry: &Entry) -> bool {
    let mut current = entry;
    for _ in 0..40 {
        if current.is_file() {
            return true;
        }
        if !current.is_symlink() {
            return false;
        }
        let Some(target) = current.link_target() else {
            return false;
        };
        let Some(next) = resolve(current.path(), target) else {
            return false;
        };
        let Some(found) = files.get(&next) else {
            return false;
        };
        current = found;
    }
    false
}

/// Joins a symlink target to its folder and normalises `.` and `..`. `None`
/// for an absolute target or one that leaves the tree.
fn resolve(link: &Path, target: &Path) -> Option<PathBuf> {
    if target.is_absolute() {
        return None;
    }
    let mut parts: Vec<&std::ffi::OsStr> = Vec::new();
    if let Some(parent) = link.parent() {
        for component in parent.components() {
            if let Component::Normal(name) = component {
                parts.push(name);
            }
        }
    }
    for component in target.components() {
        match component {
            Component::Normal(name) => parts.push(name),
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop()?;
            }
            _ => return None,
        }
    }
    Some(parts.iter().collect())
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "Source: example\n\
         Maintainer: Example <ex@example.com>\n\
         Standards-Version: 4.7.2\n\
         \n\
         Package: example\n\
         Architecture: all\n\
         Description: example\n extra\n";

    #[test]
    fn source() {
        let pass = vec![
            vec![("debian/control", TestFile::from(CONTROL))],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                (
                    "debian/install",
                    TestFile::new("usr/lib/modules-load.d/foo.conf usr/lib/modules-load.d\n"),
                ),
            ],
            // Not a direct child of debian/, or not an install file.
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                (
                    "debian/foo/install",
                    TestFile::new("debian/x /etc/modprobe.d\n"),
                ),
                ("debian/notes", TestFile::new("debian/x /etc/modprobe.d\n")),
            ],
            // Under .pc/ is skipped.
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                (
                    ".pc/x/debian/install",
                    TestFile::new("debian/x /etc/modprobe.d\n"),
                ),
            ],
            // Invalid UTF-8 yields nothing.
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                (
                    "debian/install",
                    TestFile::from_bytes(b"\xff debian/x /etc/modprobe.d\n".to_vec()),
                ),
            ],
        ];
        let fail = vec![
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                (
                    "debian/install",
                    TestFile::new("debian/x.conf /etc/modprobe.d\n"),
                ),
            ],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                (
                    "debian/example.install",
                    TestFile::new("usr/bin/x\ndebian/x.conf\tusr/lib/modprobe.d/\n"),
                ),
            ],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                (
                    "debian/install",
                    TestFile::new("debian/a /usr/lib/modprobe.d1\n"),
                ),
            ],
        ];
        SourceTreeTester::new(DhInstallInsteadOfDhInstallmodules, pass, fail).test_and_snapshot();
    }
}
