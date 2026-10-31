use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

use super::flag::emit;

pub struct UseOfCompatSymlink;

declare_rule! {
    /// The package traverses a compatibility symlink.
    ///
    /// Flags `usr/spool/`, `usr/tmp/`, `usr/{doc,bin}/X11/`, and `var/adm/`,
    /// including those directories themselves. This is a second hint: it fires
    /// in addition to whatever `files/hierarchy/standard` already flagged.
    /// Extra is empty; the Data files path (with a trailing slash for
    /// directories) is the Location. Udebs are checked too.
    UseOfCompatSymlink,
    code = "LN0063",
    tag = "use-of-compat-symlink",
    default_selected = true,
    experimental = false,
}

fn compat(name: &str) -> bool {
    name.starts_with("usr/spool/")
        || name.starts_with("usr/tmp/")
        || name.starts_with("usr/doc/X11/")
        || name.starts_with("usr/bin/X11/")
        || name.starts_with("var/adm/")
}

impl BinaryPackageRule for UseOfCompatSymlink {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter_map(|entry| {
                let path = entry.path().to_str()?;
                let name = if entry.kind() == EntryKind::Directory {
                    format!("{path}/")
                } else {
                    path.to_string()
                };
                compat(&name).then_some(PathBuf::from(name))
            })
            .collect();
        for path in paths {
            emit(ctx, path);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::BinaryPackageTester;

    use super::super::tree::tree;
    use super::*;

    #[test]
    fn test() {
        let pass = vec![
            tree(&[]),
            tree(&[("usr/bin/tool", false)]),
            tree(&[("usr/share/doc", true)]),
            tree(&[("usr/doc", true)]),
            tree(&[("usr/doc/other", false)]),
            tree(&[("var/lib", true)]),
            tree(&[("usr/tmp", false)]),
        ];
        let fail = vec![
            tree(&[("usr/spool", true)]),
            tree(&[("usr/spool/mail", false)]),
            tree(&[("usr/tmp", true)]),
            tree(&[("usr/tmp/x", false)]),
            tree(&[("usr/doc/X11", true), ("usr/doc/X11/y", false)]),
            tree(&[("usr/bin/X11", true), ("usr/bin/X11/y", false)]),
            tree(&[("var/adm", true), ("var/adm/log", false)]),
            tree(&[("usr/tmp/x", false)]).udeb(),
        ];
        BinaryPackageTester::new(UseOfCompatSymlink, pass, fail).test_and_snapshot();
    }
}
