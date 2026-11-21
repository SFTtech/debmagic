use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::ln1012_relative_conffile::{conffiles_text, parse};

pub struct UnexpectedConffile;

declare_rule! {
    /// A `conffiles` entry marked `remove-on-upgrade` is still shipped.
    ///
    /// Only entries with the `remove-on-upgrade` instruction are checked.
    /// They are reported when the path exists in the Data files (any kind of
    /// entry). Extra is the path without its leading `/`. Location is
    /// `conffiles` at that line. A non-UTF-8 member is skipped. Udebs are
    /// checked too.
    UnexpectedConffile,
    code = "LN1013",
    tag = "unexpected-conffile",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for UnexpectedConffile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(text) = conffiles_text(ctx) else {
            return;
        };
        let unexpected = {
            let Some(files) = ctx.data_files() else {
                return;
            };
            parse(&text)
                .into_iter()
                .filter(|entry| {
                    entry
                        .instructions
                        .iter()
                        .any(|instruction| instruction == "remove-on-upgrade")
                        && files.get(&entry.path).is_some()
                })
                .map(|entry| (entry.line, entry.path))
                .collect::<Vec<_>>()
        };
        for (line, path) in unexpected {
            ctx.diagnostic(Diagnostic::error(path).with_location(Location {
                path: PathBuf::from("conffiles"),
                line: Some(line),
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn removed(body: &str, shipped: &[&str]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            shipped
                .iter()
                .map(|path| ((*path).to_string(), TestFile::new("data\n")))
                .collect::<Vec<_>>(),
        )
        .control_file("conffiles", TestFile::new(body))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            removed("", &["etc/foo"]),
            removed("/etc/foo\n", &["etc/foo"]),
            removed("remove-on-upgrade /etc/gone\n", &[]),
            removed("remove-on-upgrade /etc/gone\n", &["etc/gone.conf"]),
        ];
        let fail = vec![
            removed("remove-on-upgrade /etc/some.config\n", &["etc/some.config"]),
            removed(
                "/etc/foo\n\nremove-on-upgrade /etc/gone\n",
                &["etc/foo", "etc/gone"],
            ),
            removed("remove-on-upgrade etc/gone\n", &["etc/gone"]).udeb(),
            removed("remove-on-upgrade  /etc/gone  \n", &["etc/gone"]),
        ];
        BinaryPackageTester::new(UnexpectedConffile, pass, fail).test_and_snapshot();
    }
}
