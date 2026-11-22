use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::interpreter::{Hashbang, basename};
use crate::lint::rule::BinaryPackageRule;

pub struct ScriptNotExecutable;

declare_rule! {
    /// An installed script is not an executable file.
    ///
    /// A script is a regular file or hardlink whose content starts with `#!`
    /// and names a non-empty interpreter (lincity's `#!#!#!` is not a script).
    /// It is reported when it is not an executable file. Libraries and data
    /// that are not meant to run are skipped by name, as lintian does: `.pm`,
    /// `.py`, and `ruby/*.rb` under `usr/lib/` or `usr/share/`, the debconf
    /// confmodule, `.in` files with a build placeholder interpreter, `.erb`,
    /// `.ex`, `etc/init.d/skeleton`, `etc/menu-methods`, bash completions,
    /// and `etc/X11/Xsession.d`. Python scripts under
    /// `usr/lib/python3/dist-packages/` are skipped. Paths under
    /// `usr/share/doc/` (except examples), `usr/src/`, and
    /// `usr/share/cargo/registry/` are skipped. Extra is empty; the path is
    /// the Location. Udebs are checked too.
    ScriptNotExecutable,
    code = "LN1185",
    tag = "script-not-executable",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for ScriptNotExecutable {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        for path in not_executable(files) {
            ctx.diagnostic(Diagnostic::warn(String::new()).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// Names that lintian's `script-not-executable` skips, as its regexes match them.
static SKIPPED_NAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^usr/(?:lib|share)/.*\.pm|^usr/(?:lib|share)/.*\.py|^usr/(?:lib|share)/ruby/.*\.rb|^usr/share/debconf/confmodule(?:\.sh)?$|\.in$|\.erb$|\.ex$|^etc/init\.d/skeleton$|^etc/menu-methods|^usr/share/bash-completion/completions/|^etc/X11/Xsession\.d",
    )
    .expect("valid regex")
});

/// A build placeholder interpreter such as `@PERL@` or `<<PERL>>`.
static PLACEHOLDER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:@|<<)[A-Z_]+(?:@|>>)$").expect("valid regex"));

/// Scripts that are not executable files, and are not skipped.
fn not_executable(files: &FileTree) -> Vec<PathBuf> {
    files
        .query()
        .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
        .into_iter()
        .filter_map(|entry| {
            let name = entry.path().to_str()?;
            if name.starts_with("usr/share/cargo/registry/")
                || name.starts_with("usr/src/")
                || (name.starts_with("usr/share/doc/") && !is_examples(name))
            {
                return None;
            }
            if entry.is_executable_file() {
                return None;
            }
            let bytes = files.read(entry).ok()?;
            if bytes.starts_with(b"#!#!#!") {
                return None;
            }
            let interpreter = Hashbang::read(&bytes)?.interpreter();
            if interpreter.is_empty() {
                return None;
            }
            if basename(&interpreter) == "python"
                && name.starts_with("usr/lib/python3/dist-packages/")
            {
                return None;
            }
            if name.ends_with(".in") && PLACEHOLDER.is_match(&interpreter) {
                return None;
            }
            if SKIPPED_NAME.is_match(name) {
                return None;
            }
            Some(entry.path().to_path_buf())
        })
        .collect()
}

/// `usr/share/doc/<package>/examples/…`.
fn is_examples(name: &str) -> bool {
    name.strip_prefix("usr/share/doc/")
        .and_then(|rest| rest.split_once('/'))
        .is_some_and(|(package, after)| !package.is_empty() && after.starts_with("examples/"))
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn script(path: &str, content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::new(content).mode(0o644))])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            // Executable scripts, and files that are not scripts.
            BinaryPackageCase::from(vec![(
                "usr/bin/tool",
                TestFile::new("#!/bin/sh\n").mode(0o755),
            )]),
            script("usr/share/data", "plain text\n"),
            script("usr/bin/tool", "#!\n"),
            script("usr/bin/tool", "#!#!#!\n"),
            // Skipped names and trees.
            script("usr/lib/pkg/module.pm", "#!/bin/sh\n"),
            script("usr/share/pkg/module.py", "#!/bin/sh\n"),
            script("usr/share/ruby/lib/mod.rb", "#!/usr/bin/ruby\n"),
            script("usr/share/debconf/confmodule", "#!/bin/sh\n"),
            script("usr/share/pkg/tool.in", "#!@PERL@\n"),
            script("usr/share/pkg/tool.erb", "#!/bin/sh\n"),
            script("usr/share/pkg/tool.ex", "#!/bin/sh\n"),
            script("etc/init.d/skeleton", "#!/bin/sh\n"),
            script("etc/menu-methods/tool", "#!/bin/sh\n"),
            script("usr/share/bash-completion/completions/tool", "#!/bin/sh\n"),
            script("etc/X11/Xsession.d/10tool", "#!/bin/sh\n"),
            script(
                "usr/lib/python3/dist-packages/mod.py",
                "#!/usr/bin/python3\n",
            ),
            script("usr/share/doc/pkg/tool", "#!/bin/sh\n"),
            script("usr/src/pkg/tool", "#!/bin/sh\n"),
            script("usr/share/cargo/registry/pkg/tool", "#!/bin/sh\n"),
        ];
        let fail = vec![
            script("usr/bin/tool", "#!/bin/sh\n"),
            script("usr/bin/tool", "#!/usr/bin/env python3\n"),
            // Examples are reported too.
            script("usr/share/doc/pkg/examples/tool", "#!/bin/sh\n"),
            // Placeholders are skipped only for .in files.
            script("usr/bin/tool", "#!@PERL@\n"),
            script("usr/share/pkg/tool.pl", "#!/usr/bin/perl\n"),
            script("usr/bin/tool", "#!/bin/sh\n").udeb(),
            BinaryPackageCase::from(vec![(
                "usr/bin/tool",
                TestFile::new("#!/bin/sh\n").mode(0o644).hardlink(),
            )]),
        ];
        BinaryPackageTester::new(ScriptNotExecutable, pass, fail).test_and_snapshot();
    }
}
