use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::rule::BinaryPackageRule;

pub struct ObsoleteCommandInModprobeDFile;

declare_rule! {
    /// A modprobe `.conf` file still uses `install` or `remove`.
    ///
    /// Direct children of `(etc|usr/lib)/(modprobe|modules-load).d` whose name
    /// ends in `.conf` are scanned. Each command is reported once, in the order
    /// it first appears, as the extra. The file is the Location. Udebs are
    /// checked too.
    ObsoleteCommandInModprobeDFile,
    code = "LN0414",
    tag = "obsolete-command-in-modprobe.d-file",
    default_selected = true,
    experimental = false,
}

static COMMAND: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^[ \t]*(install|remove)").expect("valid regex"));

impl BinaryPackageRule for ObsoleteCommandInModprobeDFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut hits = Vec::new();
        for entry in conf_files(files) {
            let Ok(bytes) = files.read(entry) else {
                continue;
            };
            let text = String::from_utf8_lossy(&bytes);
            for command in commands(&text) {
                hits.push((entry.path().to_path_buf(), command));
            }
        }
        for (path, command) in hits {
            ctx.diagnostic(Diagnostic::warn(command).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

fn conf_files(files: &FileTree) -> Vec<&Entry> {
    let mut entries: Vec<_> = files
        .entries()
        .iter()
        .filter(|entry| matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink))
        .filter(|entry| entry.path().to_str().is_some_and(is_conf))
        .collect();
    entries.sort_by(|left, right| left.path().cmp(right.path()));
    entries
}

fn is_conf(path: &str) -> bool {
    let Some(rest) = path
        .strip_prefix("usr/lib/")
        .or_else(|| path.strip_prefix("etc/"))
    else {
        return false;
    };
    let Some((dir, name)) = rest.split_once('/') else {
        return false;
    };
    matches!(dir, "modprobe.d" | "modules-load.d")
        && !name.is_empty()
        && !name.contains('/')
        && name.ends_with(".conf")
}

fn commands(text: &str) -> Vec<String> {
    let mut seen = Vec::new();
    for found in COMMAND.captures_iter(text) {
        let command = found[1].to_string();
        if !seen.contains(&command) {
            seen.push(command);
        }
    }
    seen
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn file(path: &str, body: &str) -> (String, TestFile) {
        (path.to_string(), TestFile::new(body))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            BinaryPackageCase::from(vec![file(
                "etc/modprobe.d/ok.conf",
                "options snd index=0\n",
            )]),
            BinaryPackageCase::from(vec![file("etc/modprobe.d/notes.txt", "install foo\n")]),
            BinaryPackageCase::from(vec![file(
                "usr/lib/modprobe.d/nested/install.conf",
                "install foo\n",
            )]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![file(
                "usr/lib/modprobe.d/lintian-install.conf",
                "remove broken\ninstall foo\ninstall again\n",
            )]),
            BinaryPackageCase::from(vec![file(
                "etc/modules-load.d/lintian-install.conf",
                "  remove me\n",
            )]),
        ];
        BinaryPackageTester::new(ObsoleteCommandInModprobeDFile, pass, fail).test_and_snapshot();
    }
}
