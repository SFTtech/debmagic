use crate::declare_rule;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::group_context::GroupContext;
use crate::lint::rule::GroupRule;

use super::gir_paths::{gir_files, gir_package_stem, location, triplet};
use crate::lint::file_tree::file_name_lossy;

pub struct GirMissingTypelibDependency;

declare_rule! {
    /// A package with GObject-Introspection XML does not depend on the
    /// package with its typelib, at its own version.
    ///
    /// Each GIR XML file (direct child of `usr/share/gir-1.0`, or of
    /// `usr/lib/<triplet>/gir-1.0` when the Architecture has a triplet) names
    /// the typelib package it expects: `gir1.2-` and its lowercased name
    /// without `.gir`, with `_` as `-`. The package passes when its
    /// Pre-Depends and Depends satisfy `<expected> (= <its version>)`, or
    /// when a `gir1.2-*` installable of the same Group provides the expected
    /// name and the package depends strictly on that installable at its
    /// version (one typelib package with `Provides` for the others). Extra is
    /// the expected name; the GIR file is the Location.
    ///
    /// A Group Rule: it needs the sibling installables, so it runs once per
    /// Group that has a `.dsc`, and reports on the installable that has the
    /// GIR file. Lintian also runs the Check on a lone `.deb`; there is no
    /// Group then, and this Rule does not run.
    GirMissingTypelibDependency,
    code = "LN1499",
    tag = "gir-missing-typelib-dependency",
    default_selected = true,
    experimental = false,
}

impl GroupRule for GirMissingTypelibDependency {
    fn run(&self, ctx: &mut GroupContext<'_>) {
        let installables = ctx.installables();
        for installable in &installables {
            let Some(files) = ctx.data_files(installable) else {
                continue;
            };
            let version = installable.version();
            let strong = installable.strong_relation();
            let mut findings = Vec::new();
            'gir: for gir in gir_files(files, triplet(installable.architecture())) {
                let expected = gir_package_stem(&file_name_lossy(gir.path()));
                for sibling in installables
                    .iter()
                    .filter(|sibling| sibling.name().starts_with("gir1.2-"))
                {
                    if sibling.relation("Provides").satisfies(&expected)
                        && strong.satisfies(&format!(
                            "{} (= {})",
                            sibling.name(),
                            sibling.version()
                        ))
                    {
                        continue 'gir;
                    }
                }
                if !strong.satisfies(&format!("{expected} (= {version})")) {
                    findings.push((location(gir), expected));
                }
            }
            for (location, expected) in findings {
                ctx.installable_diagnostic(
                    installable,
                    Diagnostic::warn(expected).with_location(location),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{GroupCase, GroupTester};

    use super::*;

    const DSC: &str = "Source: example\nVersion: 1.0\n";

    /// A development package with one GIR file.
    fn dev(name: &str, fields: &str, gir: &str) -> GroupCase {
        GroupCase::new(DSC)
            .installable(name, format!("Architecture: amd64\n{fields}"))
            .files([(gir, "<repository/>")])
    }

    #[test]
    fn group() {
        let pass = vec![
            // No GIR file.
            GroupCase::new(DSC).installable("libbad-dev", "Depends: libbad\n"),
            // The strict dependency, in Depends or Pre-Depends.
            dev(
                "libbad-dev",
                "Depends: gir1.2-bad-23 (= 1.0)\n",
                "usr/share/gir-1.0/Bad-23.gir",
            ),
            dev(
                "libbad-dev",
                "Pre-Depends: gir1.2-bad-23 (= 1.0)\n",
                "usr/share/gir-1.0/Bad_23.gir",
            ),
            dev(
                "libbad-dev",
                "Depends: gir1.2-badextras-23 (= 1.0)\n",
                "usr/lib/x86_64-linux-gnu/gir-1.0/BadExtras-23.gir",
            ),
            // A GIR file in a directory that is not looked at.
            dev("libbad-dev", "", "usr/share/doc/Bad-23.gir"),
            dev("libbad-dev", "", "usr/share/gir-1.0/Bad-23.txt"),
            // A multiarch directory of another Architecture.
            dev(
                "libbad-dev",
                "",
                "usr/lib/aarch64-linux-gnu/gir-1.0/Bad-23.gir",
            ),
            // A sibling typelib package that provides the expected name.
            dev(
                "libgood-dev",
                "Depends: gir1.2-good-42 (= 1.0)\n",
                "usr/share/gir-1.0/GoodExtras-42.gir",
            )
            .installable("gir1.2-good-42", "Provides: gir1.2-goodextras-42 (= 1.0)\n"),
        ];
        let fail = vec![
            dev("libbad-dev", "", "usr/share/gir-1.0/Bad-23.gir"),
            dev(
                "libbad-dev",
                "Depends: gir1.2-bad-23\n",
                "usr/share/gir-1.0/Bad-23.gir",
            ),
            dev(
                "libbad-dev",
                "Depends: gir1.2-bad-23 (= 2.0)\n",
                "usr/share/gir-1.0/Bad-23.gir",
            ),
            // Weak relations do not count.
            dev(
                "libbad-dev",
                "Recommends: gir1.2-bad-23 (= 1.0)\n",
                "usr/share/gir-1.0/Bad-23.gir",
            ),
            dev(
                "libbad-dev",
                "",
                "usr/lib/x86_64-linux-gnu/gir-1.0/BadExtras-23.gir",
            ),
            // Architecture: all has no multiarch directory, only the shared one.
            GroupCase::new(DSC)
                .installable("gir1.2-bad", "")
                .files([("usr/share/gir-1.0/Bad-23.gir", "<repository/>")]),
            // A sibling that provides the name, but is not depended on.
            dev("libgood-dev", "", "usr/share/gir-1.0/GoodExtras-42.gir")
                .installable("gir1.2-good-42", "Provides: gir1.2-goodextras-42 (= 1.0)\n"),
            // A sibling that does not provide the name.
            dev(
                "libgood-dev",
                "Depends: gir1.2-good-42 (= 1.0)\n",
                "usr/share/gir-1.0/GoodExtras-42.gir",
            )
            .installable("gir1.2-good-42", ""),
            // A sibling that is not a typelib package.
            dev(
                "libgood-dev",
                "Depends: other (= 1.0)\n",
                "usr/share/gir-1.0/GoodExtras-42.gir",
            )
            .installable("other", "Provides: gir1.2-goodextras-42 (= 1.0)\n"),
        ];
        GroupTester::new(GirMissingTypelibDependency, pass, fail).test_and_snapshot();
    }
}
