use crate::declare_rule;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::group_context::GroupContext;
use crate::lint::rule::GroupRule;

pub struct BinariesHaveFileConflict;

declare_rule! {
    /// Two installables built from the same source package ship the same
    /// path, and neither declares that it conflicts with or replaces the
    /// other. The extra is the two package names in sorted order and the
    /// path.
    ///
    /// Every pair of installables (not udebs) of the Group is looked at once.
    /// Two with the same name (Multi-Arch `same` builds) are skipped. The
    /// pair is exempt when either one's `Conflicts` satisfies the other's
    /// name or any name it provides, or either one's `Replaces` satisfies the
    /// other's name. A path that is a directory in both is not a conflict.
    ///
    /// A Group Rule: it needs the installables' files (the `.deb` files named
    /// in the `.changes`, or given beside it) and the Group's `.dsc`, so it
    /// does not run on a Group without a `.dsc`. The Diagnostic attaches to
    /// the Source package, as in lintian.
    BinariesHaveFileConflict,
    code = "LN1508",
    tag = "binaries-have-file-conflict",
    default_selected = true,
    experimental = false,
}

impl GroupRule for BinariesHaveFileConflict {
    fn run(&self, ctx: &mut GroupContext<'_>) {
        let installables: Vec<_> = ctx
            .installables()
            .into_iter()
            .filter(|installable| !installable.is_udeb())
            .collect();
        for (index, one) in installables.iter().enumerate() {
            let relation_one = provided_names(one);
            for two in &installables[index + 1..] {
                // Multi-Arch `same` builds have one name.
                if one.name() == two.name() {
                    continue;
                }
                let relation_two = provided_names(two);
                if two.relation("Conflicts").satisfies(&relation_one)
                    || two.relation("Replaces").satisfies(one.name())
                    || one.relation("Conflicts").satisfies(&relation_two)
                    || one.relation("Replaces").satisfies(two.name())
                {
                    continue;
                }
                let (Some(files_one), Some(files_two)) = (ctx.data_files(one), ctx.data_files(two))
                else {
                    continue;
                };
                let mut names = [one.name(), two.name()];
                names.sort_unstable();
                for entry in files_one.entries() {
                    let Some(other) = files_two.get(entry.path()) else {
                        continue;
                    };
                    if entry.is_dir() && other.is_dir() {
                        continue;
                    }
                    ctx.diagnostic(Diagnostic::warn(format!(
                        "{} {} {}",
                        names[0],
                        names[1],
                        entry.path().to_string_lossy()
                    )));
                }
            }
        }
    }
}

/// The installable's name and what it provides, as one relation text
/// (`a | b | c`).
fn provided_names(installable: &crate::lint::group_context::GroupInstallable<'_>) -> String {
    let mut names = vec![installable.name()];
    names.extend(installable.provides());
    names.join(" | ")
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{GroupCase, GroupTester, TestFile};

    use super::*;

    const DSC: &str = "Source: example\nVersion: 1.0\n";

    fn file(path: &'static str) -> Vec<(&'static str, TestFile)> {
        vec![(path, TestFile::new("x"))]
    }

    fn pair(fields_a: &str, fields_b: &str) -> GroupCase {
        GroupCase::new(DSC)
            .installable("pkga", fields_a)
            .files(file("usr/share/data"))
            .installable("pkgb", fields_b)
            .files(file("usr/share/data"))
    }

    #[test]
    fn group() {
        let pass = vec![
            GroupCase::new(DSC),
            // No shared path.
            GroupCase::new(DSC)
                .installable("pkga", "")
                .files(file("usr/share/a"))
                .installable("pkgb", "")
                .files(file("usr/share/b")),
            // A directory in both.
            GroupCase::new(DSC)
                .installable("pkga", "")
                .files([("usr/share/d", TestFile::directory())])
                .installable("pkgb", "")
                .files([("usr/share/d", TestFile::directory())]),
            // Conflicts or Replaces, either way round, or against a Provides.
            pair("", "Conflicts: pkga\n"),
            pair("Conflicts: pkgb (<< 2)\n", ""),
            pair("", "Replaces: pkga\n"),
            pair("Replaces: pkgb\n", ""),
            pair("Provides: virtual\n", "Conflicts: virtual\n"),
            pair("", "Conflicts: other, pkga\n"),
            // The same name twice (Multi-Arch same), and a udeb.
            GroupCase::new(DSC)
                .installable("pkga", "")
                .files(file("usr/share/data"))
                .installable("pkga", "")
                .files(file("usr/share/data")),
            GroupCase::new(DSC)
                .installable("pkga", "")
                .files(file("usr/share/data"))
                .udeb("pkgb-udeb", "")
                .files(file("usr/share/data")),
        ];
        let fail = vec![
            pair("", ""),
            // Replaces or Conflicts with some other package, or a weak relation.
            pair("Conflicts: other\n", "Replaces: other\n"),
            pair("", "Breaks: pkga\n"),
            // A file against a directory; each path once per pair, three packages.
            GroupCase::new(DSC)
                .installable("pkgb", "")
                .files([
                    ("usr/share/d", TestFile::new("x")),
                    ("usr/share/e", TestFile::new("x")),
                ])
                .installable("pkga", "")
                .files([
                    ("usr/share/d", TestFile::directory()),
                    ("usr/share/e", TestFile::new("y")),
                ])
                .installable("pkgc", "")
                .files(file("usr/share/e")),
        ];
        GroupTester::new(BinariesHaveFileConflict, pass, fail).test_and_snapshot();
    }
}
