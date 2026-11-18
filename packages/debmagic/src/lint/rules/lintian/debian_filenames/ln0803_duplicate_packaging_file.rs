use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::misnamed::misnamed;

pub struct DuplicatePackagingFile;

declare_rule! {
    /// A misnamed NEWS or TODO packaging file sits beside its correct name.
    ///
    /// Looks at `debian/NEWS.Debian`, `debian/NEWS.debian`, `debian/TODO.Debian`
    /// and `debian/TODO.debian` on the packaging File tree (Source tree, or
    /// Source package Patched files). Reported when the correctly named
    /// `debian/NEWS` or `debian/TODO` exists too. Extra is `better: <correct>`.
    /// The Location is the misnamed file.
    DuplicatePackagingFile,
    code = "LN0803",
    tag = "duplicate-packaging-file",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DuplicatePackagingFile {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let found = misnamed(files);
        for item in found.into_iter().filter(|item| item.duplicate) {
            ctx.diagnostic(
                Diagnostic::warn(format!("better: {}", item.correct)).with_location(Location {
                    path: item.path,
                    line: None,
                    column: None,
                }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "Source: example\n\
         Maintainer: Example <ex@example.com>\n\
         Standards-Version: 4.7.2\n\
         \n\
         Package: example\n\
         Architecture: all\n\
         Description: example\n extra\n";

    fn tree(names: &[&'static str]) -> Vec<(&'static str, TestFile)> {
        let mut files = vec![("debian/control", TestFile::new(CONTROL))];
        files.extend(names.iter().map(|name| (*name, TestFile::new("news\n"))));
        files
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            tree(&[]),
            tree(&["debian/NEWS"]),
            tree(&["debian/TODO"]),
            tree(&["debian/NEWS.Debian"]),
            tree(&["debian/TODO.debian"]),
            tree(&["debian/NEWS.Debian", "debian/TODO.debian"]),
        ];
        let fail = vec![
            tree(&["debian/NEWS", "debian/NEWS.Debian"]),
            tree(&["debian/NEWS", "debian/NEWS.debian"]),
            tree(&["debian/TODO", "debian/TODO.Debian"]),
            tree(&["debian/TODO", "debian/TODO.debian"]),
            tree(&["debian/NEWS", "debian/NEWS.Debian", "debian/NEWS.debian"]),
        ];
        SourceTreeTester::new(DuplicatePackagingFile, pass, fail).test_and_snapshot();
    }
}
