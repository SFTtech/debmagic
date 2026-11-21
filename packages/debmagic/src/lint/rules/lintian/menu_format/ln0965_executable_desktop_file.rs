use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct ExecutableDesktopFile;

declare_rule! {
    /// A desktop entry is marked executable.
    ///
    /// Checked for each `*.desktop` regular file or hardlink that is a direct
    /// child of `usr/share/applications/` or `usr/share/xsessions/`. Any
    /// execute bit counts. Templates (a name with `template` in it) are
    /// checked too, and nested files are not. Extra is the octal mode, for
    /// example `0755`. Location is the desktop file with no line. Udebs are
    /// checked too.
    ExecutableDesktopFile,
    code = "LN0965",
    tag = "executable-desktop-file",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for ExecutableDesktopFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut found: Vec<(PathBuf, u32)> = Vec::new();
        for entry in files.entries() {
            if !entry.is_executable_file() {
                continue;
            }
            let Some(path) = entry.path().to_str() else {
                continue;
            };
            let direct = ["usr/share/applications/", "usr/share/xsessions/"]
                .iter()
                .any(|dir| {
                    path.strip_prefix(dir)
                        .is_some_and(|name| !name.is_empty() && !name.contains('/'))
                });
            if direct && path.ends_with(".desktop") {
                found.push((entry.path().to_path_buf(), entry.mode()));
            }
        }
        found.sort();
        for (path, mode) in found {
            ctx.diagnostic(
                Diagnostic::error(format!("{mode:04o}")).with_location(Location {
                    path,
                    line: None,
                    column: None,
                }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn desktop(path: &str, mode: u32) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(
            path,
            TestFile::new("[Desktop Entry]\nType=Application\nName=x\nExec=x\n").mode(mode),
        )])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            desktop("usr/share/applications/a.desktop", 0o644),
            desktop("usr/share/xsessions/a.desktop", 0o644),
            // Not a desktop file, or not a direct child.
            desktop("usr/share/applications/a.sh", 0o755),
            desktop("usr/share/applications/sub/a.desktop", 0o755),
            desktop("usr/share/menu/a.desktop", 0o755),
        ];
        let fail = vec![
            desktop("usr/share/applications/a.desktop", 0o755),
            desktop("usr/share/xsessions/a.desktop", 0o744),
            desktop("usr/share/applications/a.desktop", 0o601),
            desktop("usr/share/applications/a template.desktop", 0o755),
            desktop("usr/share/applications/a.desktop", 0o755).udeb(),
        ];
        BinaryPackageTester::new(ExecutableDesktopFile, pass, fail).test_and_snapshot();
    }
}
