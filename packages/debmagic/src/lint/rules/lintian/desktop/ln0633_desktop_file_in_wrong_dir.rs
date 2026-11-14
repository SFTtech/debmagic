use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct DesktopFileInWrongDir;

declare_rule! {
    /// A desktop file is installed below `usr/share/gnome/apps`.
    ///
    /// Any entry (a file, link, or directory) whose path starts with
    /// `usr/share/gnome/apps/` and ends in `.desktop`. Extra is empty. The
    /// path is the Location. Udebs are checked too.
    DesktopFileInWrongDir,
    code = "LN0633",
    tag = "desktop-file-in-wrong-dir",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DesktopFileInWrongDir {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut paths: Vec<_> = files
            .entries()
            .iter()
            .filter(|entry| {
                entry.path().to_str().is_some_and(|path| {
                    path.starts_with("usr/share/gnome/apps/")
                        && path.len() > "usr/share/gnome/apps/".len()
                        && path.ends_with(".desktop")
                })
            })
            .map(|entry| entry.path().to_path_buf())
            .collect();
        paths.sort();
        for path in paths {
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
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn file(path: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path.to_string(), TestFile::new("[Desktop Entry]\n"))])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            file("usr/share/applications/foo.desktop"),
            file("usr/share/gnome/apps/System/foo.directory"),
            file("usr/share/gnome/apps/System/foo.desktop.bak"),
            file("usr/share/gnome/foo.desktop"),
            file("usr/local/share/gnome/apps/foo.desktop"),
        ];
        let fail = vec![
            file("usr/share/gnome/apps/System/foo.desktop"),
            file("usr/share/gnome/apps/foo.desktop"),
            file("usr/share/gnome/apps/a/b/c/foo.desktop"),
            file("usr/share/gnome/apps/System/foo.desktop").udeb(),
        ];
        BinaryPackageTester::new(DesktopFileInWrongDir, pass, fail).test_and_snapshot();
    }
}
