use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::rule::BinaryPackageRule;

pub struct MaintainerScriptEmpty;

declare_rule! {
    /// A maintainer script of the Binary package has no code.
    ///
    /// Walks the Control files whose name is `config…` or ends in `preinst`,
    /// `postinst`, `prerm`, or `postrm` (lintian matches `config` only at the
    /// start and the others only at the end) and that are regular files with a
    /// shebang naming an interpreter. After comments, blank lines, and
    /// `\` continuations are dropped, a script whose every line is dh-make
    /// boilerplate (`set -e`, the `case "$1" in` frame and its labels, `esac`,
    /// `:`, `echo "…"`, `exit N`) has no code. Applies to udebs too. Extra is
    /// empty; the Control file name is the Location.
    MaintainerScriptEmpty,
    code = "LN0014",
    tag = "maintainer-script-empty",
    default_selected = true,
    experimental = false,
}

static SCRIPT_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^config|(?:pre|post)(?:inst|rm)$").expect("valid regex"));
static ENV_PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^/usr/bin/env\s+").expect("valid regex"));

/// Lines that are boilerplate rather than code.
static BOILERPLATE: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"^\s*set\s+-\w+\s*$",
        r#"^\s*case\s+"?\$1"?\s+in\s*$"#,
        r"^\s*(?:[a-z|-]+|\*)\)\s*$",
        r"^\s*[:;]+\s*$",
        r#"^\s*echo\s+"[^"]+"(?:\s*>&2)?\s*$"#,
        r"^\s*esac\s*$",
        r"^\s*exit\s+\d+\s*$",
    ]
    .into_iter()
    .map(|pattern| Regex::new(pattern).expect("valid regex"))
    .collect()
});

static BLANK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*$").expect("valid regex"));
static COMMENT_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*#").expect("valid regex"));
static SINGLE_QUOTED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(^|[^\\](?:\\\\)*)'(?:\\.|[^\\'])+'"#).expect("valid regex"));
static DOUBLE_QUOTED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(^|[^\\](?:\\\\)*)"(?:\\.|[^\\"])+""#).expect("valid regex"));
static TRAILING_COMMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:^|[^\[\\])[\s&;()](#.*$)").expect("valid regex"));

impl BinaryPackageRule for MaintainerScriptEmpty {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.control_files() else {
            return;
        };
        let empty: Vec<_> = maintainer_scripts(files)
            .filter(|(_, content)| has_interpreter(content))
            .filter(|(_, content)| !has_code(content))
            .map(|(entry, _)| entry.path().to_path_buf())
            .collect();
        for path in empty {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// Regular Control files lintian treats as maintainer scripts, with their bytes.
fn maintainer_scripts(files: &FileTree) -> impl Iterator<Item = (&Entry, Vec<u8>)> {
    files
        .query()
        .kinds([EntryKind::RegularFile])
        .into_iter()
        .filter(|entry| {
            entry
                .path()
                .to_str()
                .is_some_and(|name| SCRIPT_NAME.is_match(name))
        })
        .filter_map(|entry| Some((entry, files.read(entry).ok()?)))
}

/// lintian's `interpreter`: the command on the shebang line, `env` stripped.
fn has_interpreter(content: &[u8]) -> bool {
    // lincity data files start with `#!#!#!`.
    if content.starts_with(b"#!#!#!") || !content.starts_with(b"#!") {
        return false;
    }
    let rest = &content[2..];
    let line = rest.split(|&byte| byte == b'\n').next().unwrap_or_default();
    let line = String::from_utf8_lossy(line);
    let line = ENV_PREFIX.replace(line.trim(), "");
    !line.is_empty()
}

/// Whether any line is more than comments and dh-make boilerplate.
fn has_code(content: &[u8]) -> bool {
    let text = String::from_utf8_lossy(content);
    let mut stashed = String::new();
    for raw in text.split('\n') {
        if BLANK.is_match(raw) || COMMENT_LINE.is_match(raw) {
            continue;
        }
        let without_comment = remove_comments(raw);
        // A trailing `\` continues the command on the next line.
        if let Some(continued) = without_comment.strip_suffix('\\') {
            stashed.push_str(continued);
            continue;
        }
        let line = std::mem::take(&mut stashed) + &without_comment;
        if !BOILERPLATE.iter().any(|pattern| pattern.is_match(&line)) {
            return true;
        }
    }
    false
}

/// Drop a trailing `# comment`, ignoring `#` inside quoted strings.
fn remove_comments(line: &str) -> String {
    if line.is_empty() {
        return String::new();
    }
    let simplified = SINGLE_QUOTED.replace_all(line, "${1}''");
    let simplified = DOUBLE_QUOTED.replace_all(&simplified, "${1}\"\"");
    match TRAILING_COMMENT
        .captures(&simplified)
        .and_then(|captures| captures.get(1))
    {
        Some(comment) => line.replacen(comment.as_str(), "", 1),
        None => line.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn with_script(name: &str, content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file(name, TestFile::new(content))
    }

    const DH_MAKE: &str = r#"#!/bin/sh
# postinst script for example
#
# see: dh_installdeb(1)

set -e

case "$1" in
    configure)
    ;;

    abort-upgrade|abort-remove|abort-deconfigure)
    ;;

    *)
        echo "postinst called with unknown argument \`$1'" >&2
        exit 1
    ;;
esac

# dh_installdeb will replace this with shell code automatically
# generated by other debhelper scripts.

#DEBHELPER#

exit 0
"#;

    #[test]
    fn test() {
        let pass = vec![
            // No Control files besides the ones that are not scripts.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("control", TestFile::new("Package: example\n"))
                .control_file("md5sums", TestFile::new("")),
            // Real code.
            with_script("postinst", "#!/bin/sh\nset -e\nupdate-foo\n"),
            with_script("config", "#!/bin/sh\n. /usr/share/debconf/confmodule\n"),
            with_script(
                "prerm",
                "#!/bin/sh\nset -e\nif [ -x /bin/foo ]; then foo; fi\n",
            ),
            with_script("postrm", "#!/bin/sh\nrm -rf /var/lib/example\n"),
            // The boilerplate frame around real code.
            with_script(
                "postinst",
                "#!/bin/sh\nset -e\ncase \"$1\" in\n  configure)\n    ldconfig\n  ;;\nesac\n",
            ),
            // No shebang: lintian cannot tell what it is.
            with_script("postinst", "set -e\n"),
            with_script("postinst", ""),
            with_script("postinst", "# only a comment\n"),
            // A shebang without an interpreter.
            with_script("postinst", "#!\n"),
            with_script("postinst", "#!"),
            with_script("postinst", "#!   \nset -e\n"),
            // lincity data files start with `#!#!#!`.
            with_script("postinst", "#!#!#!\n"),
            // Not a maintainer script name.
            with_script("triggers", "#!/bin/sh\nset -e\n"),
            with_script("preinst.d", "#!/bin/sh\nset -e\n"),
            with_script("shlibs", "#!/bin/sh\nset -e\n"),
            // A symlink is not opened.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("postinst", TestFile::symlink("foo")),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("postinst", TestFile::directory()),
            // Code before a trailing comment.
            with_script("postinst", "#!/bin/sh\nset -e\necho \"a\" ; foo # bar\n"),
            // A continuation joins the next line into one command.
            with_script("postinst", "#!/bin/sh\nset -e\nfoo \\\n  bar\n"),
            // `echo` of a variable is code.
            with_script("postinst", "#!/bin/sh\necho $FOO\n"),
            // `exit` without a number is code (lintian wants digits).
            with_script("postinst", "#!/bin/sh\nexit\n"),
            with_script("postinst", "#!/bin/sh\nexit $?\n"),
            // A `;` after `set -e` is not boilerplate.
            with_script("postinst", "#!/bin/sh\nset -e; # fail early\n"),
            // `echo` without a quoted message is not boilerplate.
            with_script("postinst", "#!/bin/sh\necho foo # comment\n"),
        ];
        let fail = vec![
            with_script("postinst", DH_MAKE),
            with_script("postinst", "#!/bin/sh\n"),
            with_script("postinst", "#!/bin/sh\nset -e\n"),
            with_script("preinst", "#!/bin/sh\n# nothing\n\n"),
            with_script("prerm", "#!/bin/sh\nset -e\nexit 0\n"),
            with_script("postrm", "#!/bin/sh\n:\n"),
            with_script("config", "#!/bin/sh\nset -e\n"),
            // `env` is stripped from the interpreter.
            with_script("postinst", "#!/usr/bin/env sh\nset -e\n"),
            // Any interpreter counts.
            with_script("postinst", "#!/usr/bin/perl\n"),
            with_script("postinst", "#! /bin/sh -e\n"),
            // Windows line endings are whitespace.
            with_script("postinst", "#!/bin/sh\r\nset -e\r\nexit 0\r\n"),
            // Comments after boilerplate, in any shape.
            with_script("postinst", "#!/bin/sh\nset -e # fail early\n"),
            with_script("postinst", "#!/bin/sh\necho \"a # b\" >&2\n"),
            // Continuations that join into boilerplate.
            with_script("postinst", "#!/bin/sh\nset \\\n -e\n"),
            // lintian's name matching is not anchored on the left for the
            // maintainer script names, nor on the right for `config`.
            with_script("my-postinst", "#!/bin/sh\nset -e\n"),
            with_script("config.sh", "#!/bin/sh\nset -e\n"),
            // Several empty scripts.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("preinst", TestFile::new("#!/bin/sh\nset -e\n"))
                .control_file("postinst", TestFile::new("#!/bin/sh\nset -e\n"))
                .control_file("postrm", TestFile::new("#!/bin/sh\nexit 0\n"))
                .control_file("prerm", TestFile::new("#!/bin/sh\nexit 0\n")),
        ];
        BinaryPackageTester::new(MaintainerScriptEmpty, pass, fail).test_and_snapshot();
    }

    #[test]
    fn comments_are_removed_outside_quotes() {
        assert_eq!(remove_comments("set -e # fail early"), "set -e ");
        assert_eq!(remove_comments("echo \"a # b\""), "echo \"a # b\"");
        assert_eq!(remove_comments("echo 'a # b' # c"), "echo 'a # b' ");
        assert_eq!(remove_comments("foo#bar"), "foo#bar");
        assert_eq!(remove_comments("[#]"), "[#]");
        assert_eq!(remove_comments(""), "");
    }

    #[test]
    fn interpreter_follows_lintian() {
        assert!(has_interpreter(b"#!/bin/sh\n"));
        assert!(has_interpreter(b"#!/usr/bin/env sh\n"));
        assert!(has_interpreter(b"#!/usr/bin/env\n"));
        assert!(!has_interpreter(b"#!\n"));
        assert!(!has_interpreter(b"#!"));
        assert!(!has_interpreter(b"#!#!#!\n"));
        assert!(!has_interpreter(b"/bin/sh\n"));
        assert!(!has_interpreter(b""));
    }
}
