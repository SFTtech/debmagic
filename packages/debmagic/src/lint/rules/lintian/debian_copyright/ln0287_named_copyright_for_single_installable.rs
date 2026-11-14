use std::path::Path;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::source_copyright;

pub struct NamedCopyrightForSingleInstallable;

declare_rule! {
    /// A source that builds one installable ships `debian/<package>.copyright`
    /// and not `debian/copyright`.
    ///
    /// Any single copyright candidate whose path is not `debian/copyright`
    /// matches, including a file reached through a symlinked `debian/`. The
    /// Location is that file. Extra is empty.
    NamedCopyrightForSingleInstallable,
    code = "LN0287",
    tag = "named-copyright-for-single-installable",
    default_selected = true,
    experimental = false,
}

impl SourceRule for NamedCopyrightForSingleInstallable {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(paths) = source_copyright::copyright_paths(ctx) else {
            return;
        };
        if paths.len() == 1 && paths[0] != Path::new("debian/copyright") {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path: paths[0].clone(),
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    const ONE: &str = "Source: example\n\nPackage: foo\nArchitecture: all\nDescription: foo\n";
    const TWO: &str = "Source: example\n\nPackage: foo\nArchitecture: all\nDescription: foo\n\nPackage: bar\nArchitecture: all\nDescription: bar\n";

    #[test]
    fn test() {
        let pass = vec![
            vec![
                ("debian", TestFile::directory()),
                ("debian/copyright", TestFile::new("Copyright 2024\n")),
            ],
            vec![
                ("debian", TestFile::directory()),
                ("debian/control", TestFile::new(ONE)),
                ("debian/copyright", TestFile::new("Copyright 2024\n")),
                ("debian/foo.copyright", TestFile::new("Copyright 2024\n")),
            ],
            vec![
                ("debian", TestFile::directory()),
                ("debian/control", TestFile::new(TWO)),
                ("debian/foo.copyright", TestFile::new("Copyright 2024\n")),
                ("debian/bar.copyright", TestFile::new("Copyright 2024\n")),
            ],
            vec![
                ("debian", TestFile::directory()),
                ("debian/control", TestFile::new(ONE)),
            ],
            vec![("README", TestFile::new("no packaging directory"))],
            // `debian/copyright` through a real directory is the usual name.
            vec![
                ("debian", TestFile::directory()),
                ("debian/copyright", TestFile::symlink("elsewhere")),
            ],
        ];
        let fail = vec![
            vec![
                ("debian", TestFile::directory()),
                ("debian/control", TestFile::new(ONE)),
                ("debian/foo.copyright", TestFile::new("Copyright 2024\n")),
            ],
            vec![
                ("debian", TestFile::directory()),
                ("debian/control", TestFile::new(TWO)),
                ("debian/foo.copyright", TestFile::new("Copyright 2024\n")),
            ],
            vec![
                ("debian", TestFile::symlink("packaging")),
                ("packaging", TestFile::directory()),
                ("packaging/copyright", TestFile::new("Copyright 2024\n")),
            ],
            vec![
                ("debian", TestFile::symlink(".")),
                ("copyright", TestFile::new("Copyright 2024\n")),
            ],
        ];
        SourceTreeTester::new(NamedCopyrightForSingleInstallable, pass, fail).test_and_snapshot();
    }
}
