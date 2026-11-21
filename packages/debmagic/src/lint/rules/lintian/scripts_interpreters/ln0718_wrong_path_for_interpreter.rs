use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::file_type::FileType;
use crate::lint::interpreter::{self, Hashbang};
use crate::lint::rule::BinaryPackageRule;

pub struct WrongPathForInterpreter;

declare_rule! {
    /// A script names a known interpreter at a path other than its table
    /// folder.
    ///
    /// Installed regular Data files whose hashbang names an interpreter in
    /// lintian's table, where the interpreter does not contain
    /// `<folder>/<basename>`. Such a file is reported only when it is
    /// executable, lies under a `bin/` or `etc/init.d/` directory, or has an
    /// absolute interpreter or calls `/usr/bin/env`. A hashbang that calls
    /// `env` is not reported. Paths under `usr/share/doc/`, `usr/src/`, and
    /// `usr/share/cargo/registry/` are skipped; examples are a different Tag.
    /// Maintainer scripts (Control files named `config…` or ending in
    /// `preinst`, `postinst`, `prerm`, or `postrm`) are checked unless they
    /// are ELF: only `sh`, `bash`, and `perl` are reported, and not under
    /// `/usr/local/`. Extra is `<interpreter> != <folder>/<basename>`; the
    /// path is the Location, or the Control file name. Udebs are checked too.
    WrongPathForInterpreter,
    code = "LN0718",
    tag = "wrong-path-for-interpreter",
    default_selected = true,
    experimental = false,
}

static MAINTAINER_SCRIPT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^config|(?:pre|post)(?:inst|rm)$").expect("valid regex"));

impl BinaryPackageRule for WrongPathForInterpreter {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let mut hits = Vec::new();
        if let Some(files) = ctx.data_files() {
            hits.extend(installed(files));
        }
        if let Some(files) = ctx.control_files() {
            hits.extend(maintainer_scripts(files));
        }
        for (path, extra) in hits {
            ctx.diagnostic(Diagnostic::error(extra).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// Installed scripts whose table interpreter is at another path.
fn installed(files: &FileTree) -> Vec<(PathBuf, String)> {
    files
        .query()
        .kinds([EntryKind::RegularFile])
        .into_iter()
        .filter_map(|entry| {
            let name = entry.path().to_str()?;
            if skipped(name) {
                return None;
            }
            let bytes = files.read(entry).ok()?;
            let hashbang = Hashbang::read(&bytes)?;
            let interpreter = hashbang.interpreter();
            // lintian skips a non-executable file unless it has an absolute
            // interpreter, calls env, or lies under bin/ or etc/init.d/.
            let in_bin = name.contains("bin/") || name.contains("etc/init.d/");
            let absolute = interpreter.starts_with('/') || hashbang.calls_env();
            if !in_bin && !absolute && !entry.is_executable_file() {
                return None;
            }
            let extra = if hashbang.calls_env() {
                None
            } else {
                let basename = interpreter::basename(&interpreter);
                let known = interpreter::lookup(basename)?;
                mismatch(&interpreter, known.folder, basename)
            }?;
            Some((entry.path().to_path_buf(), extra))
        })
        .collect()
}

/// Maintainer scripts with sh, bash, or perl outside `/usr/local/`.
fn maintainer_scripts(files: &FileTree) -> Vec<(PathBuf, String)> {
    files
        .query()
        .kinds([EntryKind::RegularFile])
        .path(&MAINTAINER_SCRIPT)
        .into_iter()
        .filter(|entry| !is_elf(entry))
        .filter_map(|entry| {
            let bytes = files.read(entry).ok()?;
            let hashbang = Hashbang::read(&bytes)?;
            // lintian reads the first word, so `/usr/bin/env` is not checked.
            if hashbang.calls_env() {
                return None;
            }
            let interpreter = hashbang.interpreter();
            if interpreter.starts_with("/usr/local/") {
                return None;
            }
            let basename = interpreter::basename(&interpreter);
            if !matches!(basename, "sh" | "bash" | "perl") {
                return None;
            }
            let known = interpreter::lookup(basename)?;
            let extra = mismatch_with(&interpreter, known.folder, basename, true)?;
            Some((entry.path().to_path_buf(), extra))
        })
        .collect()
}

/// The extra when `interpreter` is not at the table folder. A table folder
/// under `/usr/bin/`, `/usr/sbin/`, or `/usr/lib/` also accepts the path
/// without `/usr`, since Debian is usr-merged (lintian skips that for perl in
/// installed scripts). Other folders are matched as unanchored text.
fn mismatch(interpreter: &str, folder: &str, basename: &str) -> Option<String> {
    mismatch_with(interpreter, folder, basename, false)
}

fn mismatch_with(
    interpreter: &str,
    folder: &str,
    basename: &str,
    maintainer_script: bool,
) -> Option<String> {
    let expected = format!("{folder}/{basename}");
    let merged = ["/usr/bin/", "/usr/sbin/", "/usr/lib/"]
        .iter()
        .any(|prefix| expected.starts_with(prefix));
    if merged && (maintainer_script || basename != "perl") {
        let non_usr = &expected["/usr".len()..];
        let accepted = interpreter.starts_with(non_usr) || interpreter.starts_with(&expected);
        if accepted {
            return None;
        }
        // lintian prints the regex itself for maintainer scripts.
        return Some(if maintainer_script {
            format!("{interpreter} != ^(?:/usr)?{non_usr}")
        } else {
            format!("{interpreter} != {non_usr} OR {expected}")
        });
    }
    (!interpreter.contains(&expected)).then(|| format!("{interpreter} != {expected}"))
}

fn skipped(name: &str) -> bool {
    name.starts_with("usr/share/doc/")
        || name.starts_with("usr/src/")
        || name.starts_with("usr/share/cargo/registry/")
}

fn is_elf(entry: &Entry) -> bool {
    matches!(entry.file_type(), Some(FileType::Elf { .. }))
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn script(path: &str, content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::new(content).mode(0o755))])
    }

    fn maintainer(name: &str, content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file(name, TestFile::new(content))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            // The table path, or a path that contains it (usr-merge).
            script("usr/bin/tool", "#!/usr/bin/perl\n"),
            script("usr/bin/tool", "#!/bin/sh\n"),
            script("usr/bin/tool", "#!/usr/bin/sh\n"),
            script("usr/bin/tool", "#!/usr/bin/tclsh8.6\n"),
            script("usr/bin/tool", "#!/bin/ruby1.8\n"),
            script("usr/bin/tool", "#!/usr/bin/ruby1.8\n"),
            script("usr/bin/tool", "#!/usr/bin/wish8.6\n"),
            script("usr/bin/tool", "#!/usr/bin/python3 -u\n"),
            // calls env: lintian reports nothing here.
            script("usr/bin/tool", "#!/usr/bin/env perl\n"),
            script("usr/bin/tool", "#!/usr/bin/env ruby1.8\n"),
            // Not in the table, or not a script.
            script("usr/bin/tool", "#!/usr/local/bin/special-perl\n"),
            script("usr/bin/tool", "#!/usr/bin/lintian\n"),
            script("usr/bin/tool", "#!/usr/bin/python3.7\n"),
            script("usr/bin/tool", "#!/bin/env perl\n"),
            script("usr/bin/tool", "#!\n"),
            script("usr/bin/tool", "no shebang\n"),
            script("usr/bin/tool", "#!#!#!/bin/perl\n"),
            // Not executable, not under bin/ or etc/init.d/, relative.
            BinaryPackageCase::from(vec![("usr/share/foo/tool", TestFile::new("#!perl\n"))]),
            // Not a regular file: a hardlink to a wrongly placed script.
            BinaryPackageCase::from(vec![(
                "usr/bin/tool",
                TestFile::new("#!/bin/perl\n").mode(0o755).hardlink(),
            )]),
            // Skipped trees and examples, which are a different Tag.
            script("usr/share/doc/pkg/tool", "#!/bin/perl\n"),
            script("usr/share/doc/pkg/examples/tool", "#!/bin/perl\n"),
            script("usr/src/pkg/tool", "#!/bin/perl\n"),
            script("usr/share/cargo/registry/pkg/tool", "#!/bin/perl\n"),
            // ELF maintainer scripts are another Tag.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).control_file(
                "postinst",
                TestFile::new("#!/bin/perl\n").file_type(FileType::Elf {
                    has_static_symbol_table: false,
                    executable: true,
                }),
            ),
            // Maintainer scripts: env, the usr/local path, and other interpreters.
            maintainer("postinst", "#!/bin/sh\n"),
            maintainer("postinst", "#!/usr/bin/env perl\n"),
            maintainer("preinst", "#!/usr/bin/bash\n"),
            maintainer("preinst", "#!/usr/local/bin/perl\n"),
            maintainer("preinst", "#!/usr/bin/python3\n"),
            maintainer("config", "#!/usr/bin/perl\n"),
            maintainer("postinst", "#!/bin/perl\n"),
            maintainer("postrm", "#!/bin/perl\n"),
            // Not a maintainer script name.
            maintainer("templates", "#!/bin/perl\n"),
            maintainer("README", "#!/bin/perl\n"),
        ];
        let fail = vec![
            script("usr/bin/wrong-perl", "#!/bin/perl\n"),
            script("usr/bin/usr-local-perl", "#!/usr/local/bin/perl\n"),
            script("usr/bin/lefty-foo", "#!/usr/local/bin/lefty\n"),
            script("usr/bin/wrong-ruby", "#!/opt/ruby1.8\n"),
            script("usr/bin/relative-perl", "#!perl\n"),
            // Not in bin/ but absolute, or executable.
            BinaryPackageCase::from(vec![("usr/share/foo/tool", TestFile::new("#!/bin/perl\n"))]),
            BinaryPackageCase::from(vec![(
                "usr/share/foo/tool",
                TestFile::new("#!perl\n").mode(0o755),
            )]),
            // Under bin/ or etc/init.d/, executable or not.
            BinaryPackageCase::from(vec![("usr/lib/bin/tool", TestFile::new("#!/bin/perl\n"))]),
            BinaryPackageCase::from(vec![(
                "etc/init.d/tool",
                TestFile::new("#!/bin/perl\n").mode(0o755),
            )])
            .udeb(),
            BinaryPackageCase::from(vec![(
                "usr/lib/cgi-bin/tool",
                TestFile::new("#!/bin/perl\n").mode(0o755),
            )])
            .udeb(),
            // Relative interpreter that is not `env`, executable.
            script("usr/bin/relative-bash", "#!bash\n"),
            maintainer("postinst", "#!/opt/bin/perl\n"),
            maintainer("postrm", "#!/opt/bin/perl\n").udeb(),
            maintainer("prerm", "#!bash\n"),
        ];
        BinaryPackageTester::new(WrongPathForInterpreter, pass, fail).test_and_snapshot();
    }
}
