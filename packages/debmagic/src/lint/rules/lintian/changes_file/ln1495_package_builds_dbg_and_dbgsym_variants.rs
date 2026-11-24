use crate::declare_rule;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::group_context::ChangesContext;
use crate::lint::rule::ChangesRule;

pub struct PackageBuildsDbgAndDbgsymVariants;

declare_rule! {
    /// A Group builds both `<stem>-dbg` and `<stem>-dbgsym`, where `<stem>`
    /// is a package named by a `-dbgsym` `.deb` of the `Files` field. Only one
    /// of the two should be built. The extra is the two names, sorted, joined
    /// by a space: `example-dbg example-dbgsym`.
    ///
    /// The installable names are the part of each `.deb` file name before its
    /// first `_`, as lintian takes them. Lintian's pattern for a stem only
    /// matches a name ending in `-dbgsym`, so a `-dbg` package alone gives no
    /// stem, and a `-dbgsym` package with no underscore in its file name is
    /// not an installable name.
    PackageBuildsDbgAndDbgsymVariants,
    code = "LN1495",
    tag = "package-builds-dbg-and-dbgsym-variants",
    default_selected = true,
    experimental = false,
}

impl ChangesRule for PackageBuildsDbgAndDbgsymVariants {
    fn run(&self, ctx: &mut ChangesContext<'_>) {
        let installables: Vec<String> = ctx
            .changes()
            .files()
            .files()
            .iter()
            .map(|file| file.name())
            .filter(|name| name.ends_with(".deb"))
            .filter_map(|name| name.split_once('_').map(|(package, _)| package.to_string()))
            .filter(|package| !package.is_empty())
            .collect();

        let mut stems: Vec<&str> = Vec::new();
        for installable in &installables {
            if let Some(stem) = installable.strip_suffix("-dbgsym")
                && !stem.is_empty()
                && !stems.contains(&stem)
            {
                stems.push(stem);
            }
        }

        let mut messages = Vec::new();
        for stem in stems {
            let dbg = format!("{stem}-dbg");
            let dbgsym = format!("{stem}-dbgsym");
            if installables.contains(&dbg) && installables.contains(&dbgsym) {
                messages.push(format!("{dbg} {dbgsym}"));
            }
        }
        for message in messages {
            ctx.diagnostic(Diagnostic::warn(message));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{ChangesCase, ChangesTester};

    use super::*;

    fn changes(names: &[&str]) -> ChangesCase {
        let mut text = String::from("Source: example\nVersion: 1.0\nFiles:\n");
        for name in names {
            text.push_str(&format!(
                " 8de2b87704ef0a11144b57d243db88ec 12 devel optional {name}\n"
            ));
        }
        ChangesCase::from(text)
    }

    #[test]
    fn changes_file() {
        let pass = vec![
            changes(&["example-dbgsym_1.0_amd64.deb"]),
            changes(&["example-dbg_1.0_amd64.deb"]),
            changes(&["example-dbg_1.0_amd64.deb", "other-dbgsym_1.0_amd64.deb"]),
            // A udeb name does not end in `.deb`.
            changes(&[
                "example-dbg_1.0_amd64.udeb",
                "example-dbgsym_1.0_amd64.udeb",
            ]),
            // No underscore: no installable name.
            changes(&["example-dbg.deb", "example-dbgsym.deb"]),
            // Only .deb names are installables: .changes and .buildinfo are not.
            changes(&[
                "example-dbg_1.0_amd64.changes",
                "example-dbgsym_1.0_amd64.buildinfo",
            ]),
        ];
        let fail = vec![
            changes(&["example-dbg_1.0_amd64.deb", "example-dbgsym_1.0_amd64.deb"]),
            // Order in the field does not matter, and the names are sorted.
            changes(&[
                "example-dbgsym_1.0_i386.deb",
                "example-dbg_1.0_i386.deb",
                "example-dbg_1.0_amd64.deb",
            ]),
            // Two stems, one conflict each.
            changes(&[
                "a-dbg_1.0_amd64.deb",
                "a-dbgsym_1.0_amd64.deb",
                "b-dbg_1.0_amd64.deb",
                "b-dbgsym_1.0_amd64.deb",
            ]),
        ];
        ChangesTester::new(PackageBuildsDbgAndDbgsymVariants, pass, fail).test_and_snapshot();
    }
}
