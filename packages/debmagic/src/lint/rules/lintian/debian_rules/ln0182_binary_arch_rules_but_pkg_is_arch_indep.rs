use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_control::DebianControl;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::text::{complete_lines, read_rules_from_tree};

pub struct BinaryArchRulesButPkgIsArchIndep;

declare_rule! {
    /// `debian/rules` has real `binary-arch` recipe lines, but every
    /// installable is `Architecture: all`.
    ///
    /// Recipe lines of the literal target `binary-arch` are collected
    /// (comments dropped, `\` continuations joined). `dh binary-arch` and
    /// `dh $@` are no-ops and do not count. Any other recipe line does.
    /// `define` ends the target. `.PHONY` and `ifeq` do not. The package is
    /// architecture-independent when `debian/control` has at least one
    /// installable and each installable's `Architecture` is exactly `all`.
    /// Extra is empty. Location is `debian/rules` with no line number.
    /// Applies to the Source tree and Source package.
    BinaryArchRulesButPkgIsArchIndep,
    code = "LN0182",
    tag = "binary-arch-rules-but-pkg-is-arch-indep",
    default_selected = true,
    experimental = false,
}

static PHONY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:[^:]+\s)?\.PHONY(?:\s[^:]+)?:(.+)").expect("valid regex"));
static CONDITIONAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^ifn?(?:eq|def)\s").expect("valid regex"));
static TARGET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([^\s:][^:]*):+(.*)").expect("valid regex"));
static DEFINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^define ").expect("valid regex"));
static RECIPE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s+[^#]").expect("valid regex"));
/// `dh binary-arch` and `dh $@` do nothing when every package is arch-indep.
static DH_NOOP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*dh\s+(?:binary-arch|\$@)").expect("valid regex"));

impl SourceRule for BinaryArchRulesButPkgIsArchIndep {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(contents) = ctx.files().and_then(read_rules_from_tree) else {
            return;
        };
        if !binary_arch_has_substantive_rules(&contents) {
            return;
        }
        if !ctx.debian_control().is_some_and(packages_are_arch_indep) {
            return;
        }
        ctx.diagnostic(Diagnostic::warn("").with_location(Location {
            path: PathBuf::from("debian/rules"),
            line: None,
            column: None,
        }));
    }
}

fn packages_are_arch_indep(control: &DebianControl) -> bool {
    let installables = control.installables();
    !installables.is_empty()
        && installables.iter().all(|installable| {
            installable
                .fields()
                .unfolded_value("Architecture")
                .as_deref()
                == Some("all")
        })
}

fn binary_arch_has_substantive_rules(contents: &str) -> bool {
    let mut current_targets: Vec<String> = Vec::new();
    let mut saw_substantive = false;
    for (_, line) in complete_lines(contents) {
        if PHONY.is_match(&line) {
            continue;
        }
        if !CONDITIONAL.is_match(&line)
            && let Some(captures) = TARGET.captures(&line)
        {
            current_targets = captures[1].split_whitespace().map(str::to_string).collect();
            continue;
        }
        if DEFINE.is_match(&line) {
            current_targets.clear();
            continue;
        }
        if !RECIPE.is_match(&line) || !current_targets.iter().any(|target| target == "binary-arch")
        {
            continue;
        }
        if !DH_NOOP.is_match(&line) {
            saw_substantive = true;
        }
    }
    saw_substantive
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn control(body: &str) -> String {
        format!(
            "Source: example\n\
             Maintainer: Example <ex@example.com>\n\
             Standards-Version: 4.7.2\n\
             \n\
             {body}"
        )
    }

    fn one_package(architecture: &str) -> String {
        control(&format!(
            "Package: example\n\
             Architecture: {architecture}\n\
             Description: example\n extra\n"
        ))
    }

    fn tree(control_text: &str, rules_body: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/control", TestFile::new(control_text.to_string())),
            (
                "debian/rules",
                TestFile::new(format!("#!/usr/bin/make -f\n\n{rules_body}")),
            ),
        ]
    }

    fn all_arch(rules_body: &str) -> Vec<(&'static str, TestFile)> {
        tree(&one_package("all"), rules_body)
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no rules"))],
            vec![("debian/rules", TestFile::new(""))],
            vec![("debian/control", TestFile::new(one_package("all")))],
            // No binary package, so the source is not architecture-independent.
            tree(
                "Source: example\nMaintainer: Example <ex@example.com>\nStandards-Version: 4.7.2\n",
                "binary-arch:\n\techo hi\n",
            ),
            tree(&one_package("any"), "binary-arch:\n\techo hi\n"),
            tree(&one_package("amd64"), "binary-arch:\n\techo hi\n"),
            tree(&one_package("linux-any"), "binary-arch:\n\techo hi\n"),
            tree(&one_package("All"), "binary-arch:\n\techo hi\n"),
            tree(&one_package("all amd64"), "binary-arch:\n\techo hi\n"),
            // The source paragraph's Architecture is not the installable's.
            tree(
                "Source: example\n\
                 Architecture: all\n\
                 Maintainer: Example <ex@example.com>\n\
                 Standards-Version: 4.7.2\n\
                 \n\
                 Package: example\n\
                 Architecture: any\n\
                 Description: example\n extra\n",
                "binary-arch:\n\techo hi\n",
            ),
            tree(
                &control(
                    "Package: indep\n\
                     Architecture: all\n\
                     Description: indep\n extra\n\
                     \n\
                     Package: arch\n\
                     Architecture: any\n\
                     Description: arch\n extra\n",
                ),
                "binary-arch:\n\techo hi\n",
            ),
            // Rules without debian/control.
            vec![(
                "debian/rules",
                TestFile::new("#!/usr/bin/make -f\n\nbinary-arch:\n\techo hi\n"),
            )],
            all_arch(""),
            all_arch("binary-arch:\n"),
            all_arch("binary-arch:\n\n\n"),
            // `dh binary-arch` and `dh $@` are no-ops, including arguments.
            all_arch("binary-arch:\n\tdh $@\n"),
            all_arch("binary-arch:\n\tdh binary-arch\n"),
            all_arch("binary-arch:\n\tdh $@\n\tdh binary-arch --foo\n"),
            all_arch("binary-arch:\n\tdh  binary-arch\n"),
            // A comment is not a recipe. Dependencies are not recipe lines.
            all_arch("binary-arch:\n#\techo hi\n\tdh $@\n"),
            all_arch("binary-arch:\n\t# echo hi\n"),
            all_arch("binary-arch: echo hi\n"),
            // Another target, or a target that does not start in column 0.
            all_arch("binary-indep:\n\techo hi\n"),
            all_arch(" binary-arch:\n\techo hi\n"),
            all_arch("binary: binary-arch\n\techo hi\nbinary-arch:\n\tdh $@\n"),
            // `define` ends the target, so the following line is not part of it.
            all_arch("binary-arch:\n\tdh $@\ndefine FOO\n\techo hi\nendef\n"),
            // A continuation that only joins a no-op stays empty.
            all_arch("binary-arch:\n\tdh \\\n$@\n"),
        ];
        let fail = vec![
            all_arch("binary-arch:\n\techo hi\n"),
            all_arch("binary-arch:\n\tdh $@\n\techo hi\n"),
            all_arch("binary-arch:\n\t-install foo bar\n"),
            all_arch("binary-arch:\n\tdh binary-indep\n"),
            all_arch("binary-arch:\n\t/usr/bin/dh binary-arch\n"),
            all_arch("binary-arch:\n\tdh_binary-arch\n"),
            all_arch("binary-arch binary-indep:\n\techo hi\n"),
            // One report for the file, however many recipe lines.
            all_arch("binary-arch:\n\techo one\n\techo two\n"),
            // Folded Architecture is still `all`. A udeb counts.
            tree(
                &control(
                    "Package: example\n\
                     Architecture:\n all\n\
                     Description: example\n extra\n",
                ),
                "binary-arch:\n\techo hi\n",
            ),
            tree(
                &control(
                    "Package: example-udeb\n\
                     Package-Type: udeb\n\
                     Architecture: all\n\
                     Description: example\n extra\n",
                ),
                "binary-arch:\n\techo hi\n",
            ),
            tree(
                &control(
                    "Package: one\n\
                     Architecture: all\n\
                     Description: one\n extra\n\
                     \n\
                     Package: two\n\
                     Architecture: all\n\
                     Description: two\n extra\n",
                ),
                "binary-arch:\n\techo hi\n",
            ),
            // Comments and continuations: the joined line is still a recipe.
            all_arch("binary-arch:\n\techo \\\n# comment\n\thi\n"),
            all_arch("binary-\\\narch:\n\techo hi\n"),
            all_arch("binary-\\\n# comment\narch:\n\techo hi\n"),
            // `.PHONY` and `ifeq` leave `binary-arch` current.
            all_arch("binary-arch:\n.PHONY: binary-arch\n\techo hi\n"),
            all_arch("binary-arch:\nifeq ($(FOO),a:b)\n\techo hi\nendif\n"),
            // `define` does not drop lines already recorded.
            all_arch("binary-arch:\n\techo hi\ndefine FOO\n\tdh $@\n"),
            // A leading `@` is not the `dh $@` no-op.
            all_arch("binary-arch:\n\t@dh binary-arch\n"),
        ];
        SourceTreeTester::new(BinaryArchRulesButPkgIsArchIndep, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree(
            &one_package("any"),
            "binary-arch:\n\techo hi\n",
        ))];
        let fail = vec![SourcePackageCase::from(all_arch(
            "binary-arch:\n\techo hi\n",
        ))];
        SourcePackageTester::new(BinaryArchRulesButPkgIsArchIndep, pass, fail).test_and_snapshot();
    }
}
