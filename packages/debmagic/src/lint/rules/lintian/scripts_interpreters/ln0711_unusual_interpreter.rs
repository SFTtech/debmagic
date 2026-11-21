use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::interpreter::{self, Hashbang};
use crate::lint::rule::BinaryPackageRule;

pub struct UnusualInterpreter;

declare_rule! {
    /// An installed script's interpreter is neither in lintian's interpreter
    /// table nor shipped by the package.
    ///
    /// Regular Data files that start with `#!`. The interpreter is the first
    /// word of that line after a leading `/usr/bin/env`. It is unusual when its
    /// basename is not in the table (unversioned, versioned family, or a
    /// versioned name of a family), it is not `/usr/local/…`, `/bin/env`,
    /// `nodejs`, or `php…`, and no executable Data file provides it: the path
    /// without its leading slash for an absolute interpreter, or `bin/<name>`
    /// and `usr/bin/<name>` after `/usr/bin/env`. Extra is the interpreter; the
    /// path is the Location.
    ///
    /// Paths under `usr/share/doc/`, `usr/src/`, and
    /// `usr/share/cargo/registry/` are skipped, and so are the Python scripts
    /// under `usr/lib/python3/dist-packages/`, `.in` templates with a
    /// `@NAME@` or `<<NAME>>` interpreter, and scripts with an empty
    /// interpreter. A relative interpreter counts only when the path has a
    /// `bin/` or `etc/init.d/` part or the file is executable. Udebs are
    /// checked too.
    UnusualInterpreter,
    code = "LN0711",
    tag = "unusual-interpreter",
    default_selected = true,
    experimental = false,
}

static TEMPLATE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:@|<<)[A-Z_]+(?:@|>>)$").expect("valid regex"));

impl BinaryPackageRule for UnusualInterpreter {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<_> = scripts(files)
            .into_iter()
            .filter(|script| unusual(script, files))
            .collect();
        for script in hits {
            ctx.diagnostic(
                Diagnostic::warn(script.interpreter).with_location(Location {
                    path: script.path,
                    line: None,
                    column: None,
                }),
            );
        }
    }
}

struct Script {
    path: PathBuf,
    name: String,
    interpreter: String,
    calls_env: bool,
    executable: bool,
}

/// Regular files with a hashbang, outside the skipped trees.
fn scripts(files: &FileTree) -> Vec<Script> {
    files
        .query()
        .kinds([EntryKind::RegularFile])
        .into_iter()
        .filter_map(|entry| {
            let name = entry.path().to_str()?.to_string();
            if name.starts_with("usr/share/doc/")
                || name.starts_with("usr/src/")
                || name.starts_with("usr/share/cargo/registry/")
            {
                return None;
            }
            let bytes = files.read(entry).ok()?;
            let hashbang = Hashbang::read(&bytes)?;
            Some(Script {
                path: entry.path().to_path_buf(),
                name,
                interpreter: hashbang.interpreter(),
                calls_env: hashbang.calls_env(),
                executable: entry.is_executable_file(),
            })
        })
        .collect()
}

/// Whether this Tag fires for `script`.
fn unusual(script: &Script, files: &FileTree) -> bool {
    let name = script.name.as_str();
    let interpreter = script.interpreter.as_str();
    let basename = interpreter::basename(interpreter);
    let is_absolute = interpreter.starts_with('/') || script.calls_env;

    if basename == "python" && name.starts_with("usr/lib/python3/dist-packages/") {
        return false;
    }
    if name.ends_with(".in") && TEMPLATE.is_match(interpreter) {
        return false;
    }
    if !is_absolute && !name.contains("bin/") && !name.contains("etc/init.d/") && !script.executable
    {
        return false;
    }
    if interpreter.is_empty() || interpreter::lookup(basename).is_some() {
        return false;
    }
    if matches!(interpreter, "/bin/env" | "nodejs")
        || interpreter.starts_with("/usr/local/")
        || basename.starts_with("php")
    {
        return false;
    }
    !shipped(script, files)
}

/// Whether the package ships an executable file for the interpreter.
fn shipped(script: &Script, files: &FileTree) -> bool {
    let interpreter = script.interpreter.as_str();
    let candidates: Vec<String> = if let Some(path) = interpreter.strip_prefix('/') {
        vec![path.to_string()]
    } else if script.calls_env {
        vec![
            format!("bin/{interpreter}"),
            format!("usr/bin/{interpreter}"),
        ]
    } else {
        Vec::new()
    };
    candidates.iter().any(|path| {
        files
            .get(path)
            .is_some_and(|entry| entry.is_executable_file())
    })
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn script(path: &str, content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::new(content).mode(0o755))])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            // Known interpreters, including versioned names of a family.
            script("usr/bin/tool", "#!/bin/sh\n"),
            script("usr/bin/tool", "#!/usr/bin/perl\n"),
            script("usr/bin/tool", "#!/usr/bin/octave\n"),
            script("usr/bin/tool", "#!/usr/bin/octave3.0\n"),
            script("usr/bin/tool", "#!/usr/bin/env python3\n"),
            // Interpreters that another Tag or no Tag covers.
            script("usr/bin/tool", "#!/usr/local/bin/zorg\n"),
            script("usr/bin/tool", "#!/bin/env zorg\n"),
            script("usr/bin/tool", "#!/usr/bin/env nodejs\n"),
            script("usr/bin/tool", "#!/usr/bin/php\n"),
            script("usr/bin/tool", "#!/usr/bin/php7.0\n"),
            // Shipped by the package, executable.
            BinaryPackageCase::from(vec![
                (
                    "usr/bin/tool",
                    TestFile::new("#!/usr/bin/zorg\n").mode(0o755),
                ),
                ("usr/bin/zorg", TestFile::new("").mode(0o755)),
            ]),
            BinaryPackageCase::from(vec![
                (
                    "usr/bin/tool",
                    TestFile::new("#!/usr/bin/env zorg\n").mode(0o755),
                ),
                ("usr/bin/zorg", TestFile::new("").mode(0o755)),
            ]),
            BinaryPackageCase::from(vec![
                (
                    "usr/bin/tool",
                    TestFile::new("#!/usr/bin/env zorg\n").mode(0o755),
                ),
                ("bin/zorg", TestFile::new("").mode(0o755)),
            ]),
            // Not scripts.
            script("usr/bin/tool", "zorg\n"),
            script("usr/bin/tool", "#!#!#!/usr/bin/zorg\n"),
            script("usr/bin/tool", "#!\n"),
            // Relative interpreter in a non-executable file outside bin/.
            BinaryPackageCase::from(vec![("usr/lib/tool", TestFile::new("#!zorg\n"))]),
            // Skipped trees and templates.
            script("usr/share/doc/pkg/examples/tool", "#!/usr/bin/zorg\n"),
            script("usr/share/doc/pkg/tool", "#!/usr/bin/zorg\n"),
            script("usr/src/pkg/tool", "#!/usr/bin/zorg\n"),
            script("usr/share/cargo/registry/pkg/tool", "#!/usr/bin/zorg\n"),
            script("usr/lib/python3/dist-packages/tool", "#!/usr/bin/python\n"),
            script("usr/bin/tool.in", "#!@PERL@\n"),
            script("usr/share/tool.in", "#!<<PERL>>\n"),
        ];
        let fail = vec![
            script("usr/bin/tool", "#!/usr/bin/pike\n"),
            script("usr/bin/tool", "#!/usr/bin/pike7.6\n"),
            script("usr/bin/tool", "#!/usr/bin/lintian\n"),
            script("usr/bin/tool", "#!/usr/bin/env python3.7\n"),
            script("usr/bin/tool", "#!/usr/bin/env zorg\n"),
            // Not executable, but the path or the interpreter makes it a script.
            BinaryPackageCase::from(vec![("usr/bin/tool", TestFile::new("#!/usr/bin/zorg\n"))]),
            BinaryPackageCase::from(vec![("usr/share/tool", TestFile::new("#!/usr/bin/zorg\n"))]),
            BinaryPackageCase::from(vec![("etc/init.d/tool", TestFile::new("#!zorg\n"))]),
            BinaryPackageCase::from(vec![("usr/bin/tool", TestFile::new("#!zorg\n"))]),
            // A relative interpreter in an executable file outside bin/.
            script("usr/share/tool", "#!zorg\n"),
            // A shipped interpreter that is not executable, or under another path.
            BinaryPackageCase::from(vec![
                (
                    "usr/bin/tool",
                    TestFile::new("#!/usr/bin/zorg\n").mode(0o755),
                ),
                ("usr/bin/zorg", TestFile::new("")),
            ]),
            BinaryPackageCase::from(vec![
                (
                    "usr/bin/tool",
                    TestFile::new("#!/usr/bin/env zorg\n").mode(0o755),
                ),
                ("usr/local/bin/zorg", TestFile::new("").mode(0o755)),
            ]),
            script("usr/bin/tool", "#!/usr/bin/zorg\n").udeb(),
            script("usr/bin/tool", "#!/usr/bin/env python3.7\n").udeb(),
        ];
        BinaryPackageTester::new(UnusualInterpreter, pass, fail).test_and_snapshot();
    }
}
