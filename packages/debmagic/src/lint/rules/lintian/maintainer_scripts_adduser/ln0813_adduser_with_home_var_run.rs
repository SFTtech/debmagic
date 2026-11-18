use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::rule::BinaryPackageRule;

pub struct AdduserWithHomeVarRun;

declare_rule! {
    /// A maintainer script calls `adduser` with `--home /var/run`.
    ///
    /// Every maintainer script is read, with or without a hashbang. A physical
    /// line ending in `\` is joined to the next, with the `\` turned into a
    /// space. Trailing whitespace is dropped and blank lines and lines whose
    /// first character is `#` are skipped. Comments are not removed, so a
    /// match inside a trailing comment is flagged. A line is flagged when
    /// `adduser ` is followed by `--home` and then `/var/run`, with no boundary
    /// after `run`. Extra is empty. Location is the last physical line of the
    /// logical line. Udebs are checked too.
    AdduserWithHomeVarRun,
    code = "LN0813",
    tag = "adduser-with-home-var-run",
    default_selected = true,
    experimental = false,
}

static SCRIPT_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^config|(?:pre|post)(?:inst|rm)$").expect("valid regex"));
static ADDUSER_HOME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"adduser .*--home +/var/run").expect("valid regex"));

impl BinaryPackageRule for AdduserWithHomeVarRun {
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

/// Maintainer scripts and the last physical line of each logical line that
/// matches, in file-tree order.
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
        let mut continuation: Option<String> = None;
        for (index, raw) in text.split('\n').enumerate() {
            let mut line = continuation.take().unwrap_or_default();
            line.push_str(raw);
            if let Some(stripped) = line.strip_suffix('\\') {
                continuation = Some(format!("{stripped} "));
                continue;
            }
            let line = line.trim_end();
            if line.is_empty() || line.starts_with('#') || !ADDUSER_HOME.is_match(line) {
                continue;
            }
            let number = u32::try_from(index + 1).unwrap_or(u32::MAX);
            hits.push((entry.path().to_path_buf(), number));
        }
    }
    hits
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
            with_script(
                "postinst",
                "#!/bin/sh\nadduser --system foo --home /var/lib/foo\n",
            ),
            with_script("postinst", "#!/bin/sh\n# adduser --home /var/run\n"),
            with_script("postinst", "#!/bin/sh\nadduser --home /run/foo\n"),
            with_script("postinst", "#!/bin/sh\necho adduser foo\n"),
            with_script("postinst", "#!/bin/sh\nadduser --home/var/run foo\n"),
            with_script("triggers", "#!/bin/sh\nadduser --home /var/run foo\n"),
            BinaryPackageCase::from(vec![(
                "usr/bin/example",
                TestFile::new("#!/bin/sh\nadduser --home /var/run foo\n"),
            )]),
        ];
        let fail = vec![
            with_script(
                "postinst",
                "#!/bin/sh\nadduser --system --home /var/run foo\n",
            ),
            with_script(
                "postinst",
                "#!/bin/sh\nadduser --system \\\n  --home   /var/run foo\n",
            ),
            with_script("postinst", "adduser --home /var/run foo\n"),
            with_script("postinst", "#!/bin/sh\nadduser --home /var/run/x foo\n"),
            with_script("postinst", "#!/bin/sh\nadduser foo # --home /var/run\n"),
            with_script("preinst", "#!/bin/sh\nadduser --home /var/run foo\n").udeb(),
        ];
        BinaryPackageTester::new(AdduserWithHomeVarRun, pass, fail).test_and_snapshot();
    }
}
