use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct MissingConffile;

declare_rule! {
    /// A `conffiles` entry is not shipped in the package.
    ///
    /// Lines are right-trimmed and split on spaces. The last word is the path,
    /// with one leading `/` removed. Earlier words are instructions.
    /// `remove-on-upgrade` means the file should be absent. Every other entry
    /// must exist in the Data files. Extra is the path. Location is
    /// `conffiles` at that line. A non-UTF-8 member is skipped. Udebs are
    /// checked too.
    MissingConffile,
    code = "LN0424",
    tag = "missing-conffile",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MissingConffile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(control) = ctx.control_files() else {
            return;
        };
        let Some(entry) = control.get("conffiles") else {
            return;
        };
        let Ok(bytes) = control.read(entry) else {
            return;
        };
        let Ok(text) = String::from_utf8(bytes) else {
            return;
        };
        let missing = {
            let Some(files) = ctx.data_files() else {
                return;
            };
            entries(&text)
                .into_iter()
                .filter(|(_, path, instructions)| {
                    !instructions
                        .iter()
                        .any(|instruction| instruction == "remove-on-upgrade")
                        && files.get(path).is_none()
                })
                .map(|(line, path, _)| (line, path))
                .collect::<Vec<_>>()
        };
        for (line, path) in missing {
            ctx.diagnostic(Diagnostic::error(path).with_location(Location {
                path: PathBuf::from("conffiles"),
                line: Some(line),
                column: None,
            }));
        }
    }
}

fn entries(text: &str) -> Vec<(u32, String, Vec<String>)> {
    text.split('\n')
        .enumerate()
        .filter_map(|(index, line)| {
            let line = line.trim_end();
            if line.is_empty() {
                return None;
            }
            let mut words: Vec<&str> = line.split(' ').collect();
            let relative = words.pop()?;
            if relative.is_empty() {
                return None;
            }
            let relative = relative.strip_prefix('/').unwrap_or(relative);
            let instructions = words
                .into_iter()
                .filter(|word| !word.is_empty())
                .map(str::to_string)
                .collect();
            Some((
                u32::try_from(index + 1).unwrap_or(u32::MAX),
                relative.to_string(),
                instructions,
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn listed(body: &str, shipped: &[&str]) -> BinaryPackageCase {
        let mut case = BinaryPackageCase::from(
            shipped
                .iter()
                .map(|path| ((*path).to_string(), TestFile::new("data\n")))
                .collect::<Vec<_>>(),
        );
        case = case.control_file("conffiles", TestFile::new(body));
        case
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            listed("/etc/foo\n", &["etc/foo"]),
            listed("remove-on-upgrade /etc/gone\n", &[]),
            listed("", &[]),
        ];
        let fail = vec![
            listed(
                "/etc/thin-air\n/usr/gone\n/var/bye\n/etc/foo\n",
                &["etc/foo"],
            ),
            listed("/etc/missing\n", &[]).udeb(),
        ];
        BinaryPackageTester::new(MissingConffile, pass, fail).test_and_snapshot();
    }
}
