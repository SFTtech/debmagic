use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::rule::BinaryPackageRule;

pub struct MaintainerScriptCallsSystemctl;

declare_rule! {
    /// A shell maintainer script calls `systemctl`, other than `daemon-reload`.
    ///
    /// Each physical line of a maintainer script whose hashbang starts with a
    /// shell (the first word matches `^\S*sh\b`, so `/bin/sh` and `/bin/bash -e`
    /// count, `/usr/bin/env sh` does not) is checked, except lines that start
    /// with `#`. Lines are neither joined nor stripped of comments. A line is
    /// flagged when `systemctl` starts it, after whitespace or after a `;`, and
    /// the line does not contain `daemon-reload`. Extra is empty. Location is
    /// the physical line. Udebs are checked too.
    MaintainerScriptCallsSystemctl,
    code = "LN0809",
    tag = "maintainer-script-calls-systemctl",
    default_selected = true,
    experimental = false,
}

static SCRIPT_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^config|(?:pre|post)(?:inst|rm)$").expect("valid regex"));
static SHELL_HASHBANG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\S*sh\b").expect("valid regex"));
static SYSTEMCTL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:.+;)?\s*systemctl\b").expect("valid regex"));

impl BinaryPackageRule for MaintainerScriptCallsSystemctl {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.control_files() else {
            return;
        };
        let hits = calls(files);
        for (path, line) in hits {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
                line: Some(line),
                column: None,
            }));
        }
    }
}

/// Maintainer scripts with a shell hashbang and the physical lines that call
/// `systemctl`, in file-tree order.
fn calls(files: &FileTree) -> Vec<(PathBuf, u32)> {
    let mut hits = Vec::new();
    for entry in files
        .query()
        .kinds([EntryKind::RegularFile])
        .path(&SCRIPT_NAME)
        .into_iter()
    {
        let Ok(bytes) = files.read(entry) else {
            continue;
        };
        let text = String::from_utf8_lossy(&bytes);
        if !shell_hashbang(&text) {
            continue;
        }
        for (index, line) in text.split('\n').enumerate() {
            if line.starts_with('#') || !SYSTEMCTL.is_match(line) || line.contains("daemon-reload")
            {
                continue;
            }
            let number = u32::try_from(index + 1).unwrap_or(u32::MAX);
            hits.push((entry.path().to_path_buf(), number));
        }
    }
    hits
}

/// lintian's `hashbang =~ /^\S*sh\b/`. A file with no `#!` line, or one that
/// starts with `#!#!#!`, has no hashbang.
fn shell_hashbang(text: &str) -> bool {
    if text.starts_with("#!#!#!") {
        return false;
    }
    let Some(rest) = text.strip_prefix("#!") else {
        return false;
    };
    let line = rest.split('\n').next().unwrap_or("").trim();
    SHELL_HASHBANG.is_match(line)
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn with_script(name: &str, content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file(name, TestFile::new(content))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            with_script("postinst", "#!/bin/sh\nsystemctl daemon-reload\n"),
            with_script("postinst", "#!/bin/sh\n# systemctl stop foo\n"),
            with_script("postinst", "#!/bin/sh\necho systemctl stop\n"),
            with_script(
                "postinst",
                "#!/bin/sh\nsystemctl stop foo # daemon-reload\n",
            ),
            with_script("postinst", "#!/usr/bin/env sh\nsystemctl stop foo\n"),
            with_script("postinst", "#!/usr/bin/perl\nsystemctl stop foo\n"),
            with_script("postinst", "systemctl stop foo\n"),
            with_script("triggers", "#!/bin/sh\nsystemctl stop foo\n"),
            BinaryPackageCase::from(vec![(
                "usr/bin/example",
                TestFile::new("#!/bin/sh\nsystemctl stop foo\n"),
            )]),
        ];
        let fail = vec![
            with_script("postrm", "#!/bin/sh\nsystemctl stop foo\n"),
            with_script("postinst", "#!/bin/bash\nset -e\nsystemctl enable foo\n"),
            with_script("postinst", "#!/bin/sh\necho x; systemctl restart foo\n"),
            with_script("postinst", "#!/bin/sh\n  systemctl start foo\n"),
            with_script(
                "preinst",
                "#!/bin/sh\necho hi\nsystemctl stop a\nsystemctl daemon-reload\n",
            ),
            with_script("postinst", "#!/bin/sh\nsystemctl stop foo\n").udeb(),
        ];
        BinaryPackageTester::new(MaintainerScriptCallsSystemctl, pass, fail).test_and_snapshot();
    }
}
