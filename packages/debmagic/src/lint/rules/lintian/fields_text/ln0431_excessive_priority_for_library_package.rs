use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::{BinaryPackageContext, SourceContext};
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, SourceRule};

pub struct ExcessivePriorityForLibraryPackage;

declare_rule! {
    /// A library package has `Priority` `required`, `important`, or `standard`.
    ///
    /// The package name starts with `lib`, does not end in `-bin`, and is not
    /// `libc` plus digits and dots. `Section` is exactly `libs` or `libdevel`.
    /// `Priority: extra` is treated as `optional` and does not match. The
    /// source paragraph and Installation control are both read. Extra is the
    /// priority after that remap. Udebs are checked too.
    ExcessivePriorityForLibraryPackage,
    code = "LN0431",
    tag = "excessive-priority-for-library-package",
    default_selected = true,
    experimental = false,
}

static LIBC: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^libc[0-9.]+$").expect("valid regex"));

impl SourceRule for ExcessivePriorityForLibraryPackage {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let name = control
            .source()
            .get("Source")
            .filter(|name| !name.is_empty())
            .map(str::to_string)
            .or_else(|| {
                ctx.debian_changelog().and_then(|changelog| {
                    changelog
                        .entries()
                        .first()
                        .map(|entry| entry.source().to_string())
                        .filter(|name| !name.is_empty())
                })
            })
            .unwrap_or_default();
        if let Some(priority) = excessive(&name, control.source()) {
            ctx.diagnostic(Diagnostic::warn(priority));
        }
    }
}

impl BinaryPackageRule for ExcessivePriorityForLibraryPackage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let name = ctx.package_name().to_string();
        if let Some(priority) = excessive(&name, ctx.installation_control().fields()) {
            ctx.diagnostic(Diagnostic::warn(priority));
        }
    }
}

fn excessive(name: &str, fields: &ControlParagraph) -> Option<String> {
    if !fields.declares("Priority") {
        return None;
    }
    let mut priority = fields.unfolded_value("Priority")?;
    if priority == "extra" {
        priority = "optional".to_string();
    }
    if !name.starts_with("lib") || name.ends_with("-bin") || LIBC.is_match(name) {
        return None;
    }
    let section = fields.unfolded_value("Section").unwrap_or_default();
    if !matches!(section.as_str(), "libs" | "libdevel") {
        return None;
    }
    matches!(priority.as_str(), "required" | "important" | "standard").then_some(priority)
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn source(name: &str, fields: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: {name}\n{fields}\n\nPackage: {name}\nArchitecture: all\nDescription: d\n more\n"
            )),
        )]
    }

    fn deb(name: &str, section: &str, priority: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .package(name)
            .field("Section", section)
            .field("Priority", priority)
    }

    #[test]
    fn source_tree() {
        let pass = vec![
            source("libfoo", "Section: libs\nPriority: optional\n"),
            source("libfoo-bin", "Section: libs\nPriority: required\n"),
            source("libc6", "Section: libs\nPriority: required\n"),
            source("libfoo", "Section: misc\nPriority: required\n"),
            source("libfoo", "Section: libs\nPriority: extra\n"),
            source("foo", "Section: libs\nPriority: standard\n"),
        ];
        let fail = vec![
            source("libfoo", "Section: libs\nPriority: important\n"),
            source("libfoo-dev", "Section: libdevel\nPriority: standard\n"),
            source("libfoo", "Section: libs\nPriority: required\n"),
        ];
        SourceTreeTester::new(ExcessivePriorityForLibraryPackage, pass, fail).test_and_snapshot();
    }

    #[test]
    fn binary() {
        let pass = vec![
            deb("libfoo", "libs", "optional"),
            deb("libfoo-bin", "libs", "required"),
            deb("libc6.1", "libs", "important"),
            deb("libfoo", "libs", "extra").udeb(),
        ];
        let fail = vec![
            deb("libfoo", "libs", "important"),
            deb("libfoo-dev", "libdevel", "standard"),
            deb("libfoo", "libs", "required").udeb(),
        ];
        BinaryPackageTester::new(ExcessivePriorityForLibraryPackage, pass, fail)
            .test_and_snapshot_with_suffix("binary");
    }
}
