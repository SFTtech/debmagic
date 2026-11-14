use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::SourceRule;

use super::source_copyright;

pub struct DebianCopyrightIsSymlink;

declare_rule! {
    /// `debian/copyright` or `debian/<installable>.copyright` is a symlink.
    ///
    /// Every candidate that is a symlink is reported; the directory itself is
    /// followed when `debian` is a symlink. Extra is empty; the symlink path
    /// is the Location.
    DebianCopyrightIsSymlink,
    code = "LN0608",
    tag = "debian-copyright-is-symlink",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DebianCopyrightIsSymlink {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(paths) = source_copyright::copyright_paths(ctx) else {
            return;
        };
        let Some(files) = ctx.files() else {
            return;
        };
        let symlinks: Vec<_> = paths
            .into_iter()
            .filter(|path| {
                files
                    .get(path)
                    .is_some_and(|entry| entry.kind() == EntryKind::Symlink)
            })
            .collect();
        for path in symlinks {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
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

    const CONTROL: &str = "Source: example\n\nPackage: foo\nArchitecture: all\nDescription: foo\n";

    #[test]
    fn test() {
        let pass = vec![
            vec![
                ("debian", TestFile::directory()),
                ("debian/copyright", TestFile::new("Copyright 2024\n")),
            ],
            vec![("README", TestFile::new("no packaging directory"))],
            Vec::new(),
        ];
        let fail = vec![
            vec![
                ("debian", TestFile::directory()),
                ("debian/copyright", TestFile::symlink("example.copyright")),
                (
                    "debian/example.copyright",
                    TestFile::new("Copyright 2024\n"),
                ),
            ],
            vec![
                ("debian", TestFile::directory()),
                ("debian/control", TestFile::new(CONTROL)),
                ("debian/copyright", TestFile::new("Copyright 2024\n")),
                ("debian/foo.copyright", TestFile::symlink("copyright")),
            ],
            vec![
                ("debian", TestFile::directory()),
                ("debian/copyright", TestFile::symlink("elsewhere")),
            ],
        ];
        SourceTreeTester::new(DebianCopyrightIsSymlink, pass, fail).test_and_snapshot();
    }
}
