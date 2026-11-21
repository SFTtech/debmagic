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

use super::scan::{script_entries, shebang_token};

pub struct InterpreterNotAbsolute;

declare_rule! {
    /// A script's interpreter is a relative path, such as `#!perl`.
    ///
    /// Installed scripts report the interpreter after a leading
    /// `/usr/bin/env` is removed. `/usr/bin/env` itself is absolute, so
    /// `#!/usr/bin/env perl` is not reported, but `#!env perl` is. Maintainer
    /// scripts report the first word of the shebang, which keeps `env`. ELF
    /// maintainer scripts and scripts with no interpreter are skipped. Extra
    /// is the interpreter; the path is the Location.
    ///
    /// An installed relative-interpreter script is reported only when it is
    /// executable, or lies under a `bin/` or `etc/init.d/` path. Installed
    /// files under `usr/share/doc/` and `usr/src/` are skipped, since their
    /// examples are a different Tag, as are `usr/share/cargo/registry/`. A
    /// `.in` file whose interpreter is a template token such as `@PERL@` is
    /// skipped, and so is a `python` in `usr/lib/python3/dist-packages/`.
    /// Udebs are checked too.
    InterpreterNotAbsolute,
    code = "LN0986",
    tag = "interpreter-not-absolute",
    default_selected = true,
    experimental = false,
}

static TEMPLATE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:@|<<)[A-Z_]+(?:@|>>)$").expect("valid regex"));

impl BinaryPackageRule for InterpreterNotAbsolute {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let mut hits = Vec::new();
        if let Some(files) = ctx.data_files() {
            hits.extend(installed(files));
        }
        if let Some(files) = ctx.control_files() {
            hits.extend(maintainer_scripts(files));
        }
        for (path, extra) in hits {
            ctx.diagnostic(Diagnostic::warn(extra).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// Installed scripts whose interpreter is relative.
fn installed(files: &FileTree) -> Vec<(PathBuf, String)> {
    files
        .query()
        .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
        .into_iter()
        .filter_map(|entry| {
            let name = entry.path().to_str()?;
            if name.starts_with("usr/share/doc/")
                || name.starts_with("usr/src/")
                || name.starts_with("usr/share/cargo/registry/")
            {
                return None;
            }
            let bytes = files.read(entry).ok()?;
            let hashbang = Hashbang::read(&bytes)?;
            let interpreter = hashbang.interpreter();
            if interpreter.is_empty() {
                return None;
            }
            if name.ends_with(".in") && TEMPLATE.is_match(&interpreter) {
                return None;
            }
            if interpreter::basename(&interpreter) == "python"
                && name.starts_with("usr/lib/python3/dist-packages/")
            {
                return None;
            }
            let absolute = interpreter.starts_with('/') || hashbang.calls_env();
            // lintian skips a non-executable file unless it is under bin/ or
            // etc/init.d/ or has an absolute interpreter.
            let in_bin = name.contains("bin/") || name.contains("etc/init.d/");
            if absolute || (!in_bin && !entry.is_executable_file()) {
                return None;
            }
            Some((entry.path().to_path_buf(), interpreter))
        })
        .collect()
}

/// Maintainer scripts whose first shebang word is relative.
fn maintainer_scripts(files: &FileTree) -> Vec<(PathBuf, String)> {
    script_entries(files)
        .into_iter()
        .filter(|(entry, _)| !is_elf(entry))
        .filter_map(|(entry, bytes)| {
            let interpreter = shebang_token(&bytes)?;
            (!interpreter.starts_with('/')).then(|| (entry.path().to_path_buf(), interpreter))
        })
        .collect()
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
            script("usr/bin/tool", "#!/bin/sh\n"),
            script("usr/bin/tool", "#!/usr/bin/env perl\n"),
            script("usr/bin/tool", "no shebang\n"),
            // Templates and the usr-lib python in dist-packages.
            script("usr/lib/tool.in", "#!@PERL@\n"),
            script("usr/lib/python3/dist-packages/tool.py", "#!python\n"),
            // Skipped trees.
            script("usr/share/doc/pkg/tool", "#!perl\n"),
            script("usr/share/doc/pkg/examples/tool", "#!perl\n"),
            script("usr/src/pkg/tool", "#!perl\n"),
            script("usr/share/cargo/registry/pkg/tool", "#!perl\n"),
            // Not executable and outside bin/ and etc/init.d/: not a script.
            BinaryPackageCase::from(vec![("usr/share/foo/tool", TestFile::new("#!perl\n"))]),
            // Maintainer scripts: absolute, ELF, not a maintainer script name.
            maintainer("postinst", "#!/bin/sh\n"),
            maintainer("postinst", "#!/usr/bin/env perl\n"),
            maintainer("config", "#!/bin/sh\n"),
            maintainer("README", "#!perl\n"),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).control_file(
                "postinst",
                TestFile::new("#!sh\n").file_type(FileType::Elf {
                    has_static_symbol_table: false,
                    executable: true,
                }),
            ),
        ];
        let fail = vec![
            script("usr/bin/tool", "#!perl\n"),
            script("usr/bin/tool", "#!env perl\n"),
            // Executable, or under bin/ or etc/init.d/, even when not executable.
            script("usr/share/foo/tool", "#!perl\n"),
            BinaryPackageCase::from(vec![("usr/bin/tool", TestFile::new("#!perl\n"))]),
            BinaryPackageCase::from(vec![("etc/init.d/tool", TestFile::new("#!perl\n"))]),
            script("usr/bin/tool", "#!perl\n").udeb(),
            maintainer("postinst", "#!sh\n"),
            maintainer("preinst", "#!python3 -u\n"),
        ];
        BinaryPackageTester::new(InterpreterNotAbsolute, pass, fail).test_and_snapshot();
    }
}
