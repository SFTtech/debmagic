use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::source_copyright;

pub struct NoDebianCopyrightInSource;

declare_rule! {
    /// The source has a `debian/` directory and none of `debian/copyright` or
    /// `debian/<installable>.copyright`.
    ///
    /// A symlink counts as present. The directory is followed when `debian`
    /// itself is a symlink. Extra and Location are empty.
    NoDebianCopyrightInSource,
    code = "LN0281",
    tag = "no-debian-copyright-in-source",
    default_selected = true,
    experimental = false,
}

impl SourceRule for NoDebianCopyrightInSource {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(paths) = source_copyright::copyright_paths(ctx) else {
            return;
        };
        if paths.is_empty() {
            ctx.diagnostic(Diagnostic::warn(""));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "Source: example\n\nPackage: foo\nArchitecture: all\nDescription: foo\n";

    #[test]
    fn test() {
        let pass = vec![
            vec![
                ("debian", TestFile::directory()),
                ("debian/copyright", TestFile::new("Copyright 2024\n")),
            ],
            vec![
                ("debian", TestFile::directory()),
                ("debian/control", TestFile::new(CONTROL)),
                ("debian/foo.copyright", TestFile::new("Copyright 2024\n")),
            ],
            vec![
                ("debian", TestFile::directory()),
                ("debian/copyright", TestFile::symlink("elsewhere")),
            ],
            vec![("README", TestFile::new("no packaging directory"))],
            Vec::new(),
        ];
        let fail = vec![
            vec![
                ("debian", TestFile::directory()),
                ("debian/control", TestFile::new(CONTROL)),
            ],
            vec![
                ("debian", TestFile::directory()),
                ("debian/rules", TestFile::new("#!/usr/bin/make -f\n")),
            ],
            vec![("debian", TestFile::directory())],
        ];
        SourceTreeTester::new(NoDebianCopyrightInSource, pass, fail).test_and_snapshot();
    }
}
