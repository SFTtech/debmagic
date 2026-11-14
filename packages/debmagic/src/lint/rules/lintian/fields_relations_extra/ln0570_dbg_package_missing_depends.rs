use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_control::DebianControl;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::is_arch_all;

pub struct DbgPackageMissingDepends;

declare_rule! {
    /// A `-dbg` package does not depend on an architecture-dependent package of the source.
    ///
    /// Read from the installable paragraphs of `debian/control`. Installables
    /// named `*-dbg` are the debug packages; every other installable whose
    /// `Architecture` is not `all` is architecture dependent. A debug package
    /// is missing its dependency when `Pre-Depends` and `Depends` together
    /// name none of the architecture-dependent installables. A transitional
    /// package (Description) may instead depend on another `*-dbg`. Extra is
    /// the debug package name.
    DbgPackageMissingDepends,
    code = "LN0570",
    tag = "dbg-package-missing-depends",
    default_selected = true,
    experimental = false,
}

static TRANSITIONAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)transitional package").expect("valid regex"));

impl SourceRule for DbgPackageMissingDepends {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let messages = ctx.debian_control().map(messages).unwrap_or_default();
        for message in messages {
            ctx.diagnostic(Diagnostic::warn(message));
        }
    }
}

fn messages(control: &DebianControl) -> Vec<String> {
    let arch_dependent: Vec<&str> = control
        .installables()
        .iter()
        .filter(|installable| {
            !installable.name().ends_with("-dbg") && !is_arch_all(installable.fields())
        })
        .map(|installable| installable.name())
        .collect();
    control
        .installables()
        .iter()
        .filter(|installable| installable.name().ends_with("-dbg"))
        .filter(|installable| {
            let strong = installable
                .fields()
                .relations_and(&["Pre-Depends", "Depends"]);
            let names: Vec<&str> = strong.alternatives().map(|a| a.package()).collect();
            if names.iter().any(|name| arch_dependent.contains(name)) {
                return false;
            }
            let transitional = installable
                .fields()
                .get("Description")
                .is_some_and(|description| TRANSITIONAL.is_match(description));
            !(transitional && names.iter().any(|name| name.ends_with("-dbg")))
        })
        .map(|installable| installable.name().to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::super::testing::raw;
    use super::*;

    #[test]
    fn test() {
        let pass = vec![
            Vec::<(&str, TestFile)>::new(),
            raw("Source: example\n\nPackage: example\nArchitecture: any\n"),
            raw("Source: example\n\nPackage: example\nArchitecture: any\n\n\
                 Package: example-dbg\nArchitecture: any\nDepends: example (= ${binary:Version})\n"),
            raw("Source: example\n\nPackage: example\nArchitecture: any\n\n\
                 Package: example-dbg\nArchitecture: any\nPre-Depends: foo | example\n"),
            raw("Source: example\n\nPackage: example\nArchitecture: any\n\n\
                 Package: example-old-dbg\nArchitecture: any\n\
                 Description: transitional package\n transitional\nDepends: example-dbg\n\n\
                 Package: example-dbg\nArchitecture: any\nDepends: example\n"),
        ];
        let fail = vec![
            raw("Source: example\n\nPackage: example\nArchitecture: any\n\n\
                 Package: example-dbg\nArchitecture: any\nDepends: ${misc:Depends}\n"),
            // Only an architecture-independent package to depend on.
            raw(
                "Source: example\n\nPackage: example-data\nArchitecture: all\n\n\
                 Package: example-dbg\nDepends: example-data\n",
            ),
            // The alternative must be a whole package name, and recommends is weak.
            raw("Source: example\n\nPackage: example\nArchitecture: any\n\n\
                 Package: example-dbg\nRecommends: example\nDepends: example2\n\n\
                 Package: other-dbg\nDepends: other-dbg2\n"),
            // Only a transitional package may depend on another -dbg.
            raw("Source: example\n\nPackage: example\nArchitecture: any\n\n\
                 Package: example-old-dbg\nDescription: old\n x\nDepends: example-dbg\n\n\
                 Package: example-dbg\nDepends: example\n"),
        ];
        SourceTreeTester::new(DbgPackageMissingDepends, pass, fail).test_and_snapshot();
    }
}
