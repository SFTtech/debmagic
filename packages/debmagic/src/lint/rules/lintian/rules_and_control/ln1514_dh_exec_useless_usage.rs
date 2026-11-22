use std::path::{Path, PathBuf};

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

use super::dh_exec::dh_exec_scripts;

pub struct DhExecUselessUsage;

declare_rule! {
    /// A dh-exec script among the `debian/` `install` or `manpages` files
    /// uses dh-exec for nothing dh-exec adds: no rename (`=>`), no filter
    /// (`[...]` or `<...>`), and no `usr/lib/${...}` line that is more than a
    /// plain wildcard. The package should use a wildcard instead. The extra is
    /// empty and the Location is the file.
    ///
    /// A `usr/lib/${DEB_BUILD_*}` or `usr/lib/${DEB_HOST_*}` line counts as a
    /// plain wildcard only when it has no second variable that differs, as
    /// lintian's `$dhe_needed` works out (compat 12 and below). Only the
    /// `install` and `manpages` files are reported, as in lintian. Info
    /// severity and not selected by default.
    ///
    /// Differs from lintian: the file is read as UTF-8, a file that is not
    /// UTF-8 is not looked at.
    DhExecUselessUsage,
    code = "LN1514",
    tag = "dh-exec-useless-usage",
    default_selected = false,
    experimental = false,
}

impl SourceRule for DhExecUselessUsage {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        for script in dh_exec_scripts(ctx) {
            if script.needed || !is_install_or_manpages(&script.path) {
                continue;
            }
            report(ctx, script.path);
        }
    }
}

/// lintian's `$item =~ /debian\/.*(install|manpages)/`: the name has
/// `debian/` followed somewhere by `install` or `manpages`.
fn is_install_or_manpages(path: &Path) -> bool {
    let name = path.to_string_lossy();
    name.find("debian/").is_some_and(|at| {
        let rest = &name[at + "debian/".len()..];
        rest.contains("install") || rest.contains("manpages")
    })
}

fn report(ctx: &mut SourceContext<'_>, path: PathBuf) {
    ctx.diagnostic(
        Diagnostic::warn("")
            .with_severity(Severity::Info)
            .with_location(Location {
                path,
                line: None,
                column: None,
            }),
    );
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "\
Source: example\n\
Maintainer: Example <ex@example.com>\n\
Standards-Version: 4.7.2\n\
\n\
Package: example\n\
Architecture: all\n\
Description: example\n extra\n";

    const DH: &str = "%:\n\tdh $@\n";

    fn package(compat: &str, files: &[(&'static str, &str)]) -> Vec<(&'static str, TestFile)> {
        let mut tree = vec![
            ("debian/control", TestFile::new(CONTROL)),
            ("debian/rules", TestFile::new(DH)),
            ("debian/compat", TestFile::new(compat)),
        ];
        for (path, text) in files {
            tree.push((path, TestFile::new(*text)));
        }
        tree
    }

    #[test]
    fn source() {
        let pass = vec![
            // A rename, a filter, or a second variable is a feature.
            package(
                "12\n",
                &[("debian/install", "#!/usr/bin/dh-exec\nfoo => bar\n")],
            ),
            package(
                "12\n",
                &[("debian/install", "#!/usr/bin/dh-exec\nfoo [linux-any]\n")],
            ),
            package(
                "12\n",
                &[(
                    "debian/install",
                    "#!/usr/bin/dh-exec\nusr/lib/${DEB_HOST_MULTIARCH} /usr/lib/${DEB_HOST_ARCH}\n",
                )],
            ),
            // A feature on any line keeps the file.
            package(
                "12\n",
                &[(
                    "debian/pkg.manpages",
                    "#!/usr/bin/dh-exec\nusr/lib/${DEB_HOST_MULTIARCH}\nfoo => bar\n",
                )],
            ),
            // Not an install or manpages file.
            package(
                "12\n",
                &[(
                    "debian/links",
                    "#!/usr/bin/dh-exec\nusr/lib/${DEB_HOST_MULTIARCH}\n",
                )],
            ),
            // Not a dh-exec script.
            package(
                "12\n",
                &[("debian/install", "usr/lib/${DEB_HOST_MULTIARCH}\n")],
            ),
        ];
        let fail = vec![
            // A plain wildcard in disguise.
            package(
                "12\n",
                &[(
                    "debian/install",
                    "#!/usr/bin/dh-exec\nusr/lib/${DEB_HOST_MULTIARCH}\n",
                )],
            ),
            package(
                "12\n",
                &[(
                    "debian/pkg.manpages",
                    "#!/usr/bin/dh-exec\nusr/lib/${DEB_HOST_MULTIARCH}/\n",
                )],
            ),
            // Compat 13 does not count the wildcard line as a feature.
            package(
                "13\n",
                &[(
                    "debian/install",
                    "#!/usr/bin/dh-exec\nusr/lib/${DEB_HOST_MULTIARCH}\n",
                )],
            ),
        ];
        SourceTreeTester::new(DhExecUselessUsage, pass, fail).test_and_snapshot();
    }
}
