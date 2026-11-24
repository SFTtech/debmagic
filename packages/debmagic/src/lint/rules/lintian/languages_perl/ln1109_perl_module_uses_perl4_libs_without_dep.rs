use std::os::unix::ffi::OsStrExt;
use std::sync::LazyLock;

use regex::bytes::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct PerlModuleUsesPerl4LibsWithoutDep;

declare_rule! {
    /// A Perl module (`.pm`) does `do` or `require` one of the Perl 4 libraries
    /// by its `.pl` name, and the package does not depend on
    /// `libperl4-corelibs-perl | perl (<< 5.12.3-7)`. Those libraries were
    /// deprecated in Perl 5.14.
    ///
    /// Only regular files and hardlinks are read, and only when the strong
    /// relation (`Pre-Depends` and `Depends`, AND-ed) does not already satisfy
    /// the condition. Each matching line is reported once, with the module
    /// name (`assert.pl`) as extra and the line as Location.
    PerlModuleUsesPerl4LibsWithoutDep,
    code = "LN1109",
    tag = "perl-module-uses-perl4-libs-without-dep",
    default_selected = true,
    experimental = false,
}

const PERL4_CONDITION: &str = "libperl4-corelibs-perl | perl (<< 5.12.3-7)";

static PERL4_LIBRARY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?-u)(?:do|require)\s+['"](abbrev|assert|bigfloat|bigint|bigrat|cacheout|complete|ctime|dotsh|exceptions|fastcwd|find|finddepth|flush|getcwd|getopt|getopts|hostname|importenv|look|newgetopt|open2|open3|pwd|shellwords|stat|syslog|tainted|termcap|timelocal|validate)\.pl['"]"#,
    )
    .expect("valid regex")
});

impl BinaryPackageRule for PerlModuleUsesPerl4LibsWithoutDep {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let strong = ctx
            .installation_control()
            .fields()
            .relations_and(&["Pre-Depends", "Depends"]);
        if strong.satisfies(PERL4_CONDITION) {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut diagnostics = Vec::new();
        for entry in files.entries() {
            if !entry.is_file() || !entry.path().as_os_str().as_bytes().ends_with(b".pm") {
                continue;
            }
            let Ok(bytes) = files.read(entry) else {
                continue;
            };
            for (index, line) in bytes.split(|byte| *byte == b'\n').enumerate() {
                let Some(captures) = PERL4_LIBRARY.captures(line) else {
                    continue;
                };
                let module = String::from_utf8_lossy(&captures[1]);
                diagnostics.push(Diagnostic::warn(format!("{module}.pl")).with_location(
                    Location {
                        path: entry.path().to_path_buf(),
                        line: Some(index as u32 + 1),
                        column: None,
                    },
                ));
            }
        }
        for diagnostic in diagnostics {
            ctx.diagnostic(diagnostic);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    const DO_ASSERT: &str =
        "package Naughty;\nuse strict;\nuse warnings;\n\nrequire 'assert.pl';\n\n1;\n";

    fn module(path: &str, body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::new(body))])
    }

    fn with_depends(depends: &str) -> BinaryPackageCase {
        module("usr/share/perl5/Naughty.pm", DO_ASSERT).field("Depends", depends)
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            with_depends("libperl4-corelibs-perl | perl (<< 5.12.3-7)"),
            BinaryPackageCase::from(vec![(
                "usr/share/perl5/Naughty.pm",
                TestFile::new(DO_ASSERT),
            )])
            .field("Pre-Depends", "libperl4-corelibs-perl | perl (<< 5.12.3-7)"),
            // Not a `.pm`: lintian only reads modules.
            module("usr/share/perl5/Naughty.pl", DO_ASSERT),
            module("usr/share/perl5/Naughty.pm", "require 'strict.pm';\n"),
            module("usr/share/perl5/Naughty.pm", "require 'assert.pl2';\n"),
            module("usr/share/perl5/Naughty.pm", "require assert.pl;\n"),
            // Symlinks are not files.
            BinaryPackageCase::from(vec![("usr/share/perl5/Naughty.pm", TestFile::symlink("x"))]),
        ];
        let fail = vec![
            with_depends("perl"),
            module("usr/share/perl5/Naughty.pm", DO_ASSERT),
            module("usr/share/perl5/Naughty.pm", "do \"find.pl\";\n"),
            module("usr/share/perl5/Naughty.pm", "do 'finddepth.pl';\n"),
            module("usr/share/perl5/Naughty.pm", "undo 'stat.pl';\n"),
            module("usr/share/perl5/Naughty.pm", "require   'getopts.pl';\n"),
            module("usr/share/perl5/Naughty.pm", "1;\nrequire 'assert.pl';\n"),
            module("usr/share/perl5/Naughty.pm", DO_ASSERT).udeb(),
            BinaryPackageCase::from(vec![(
                "usr/share/perl5/Naughty.pm",
                TestFile::new(DO_ASSERT).hardlink(),
            )]),
            BinaryPackageCase::from(vec![(
                OsString::from_vec(b"usr/share/perl5/Na\xffughty.pm".to_vec()),
                TestFile::new(DO_ASSERT),
            )]),
        ];
        BinaryPackageTester::new(PerlModuleUsesPerl4LibsWithoutDep, pass, fail).test_and_snapshot();
    }
}
