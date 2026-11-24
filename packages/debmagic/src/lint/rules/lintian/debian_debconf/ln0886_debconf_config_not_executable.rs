use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct DebconfConfigNotExecutable;

declare_rule! {
    /// The `config` control file is a regular file with no execute bit.
    ///
    /// Only a regular file or hardlink is checked; a symlink, a directory, or
    /// a missing `config` is not reported. Any execute bit (`0111`) passes.
    /// The Extra is empty and there is no Location: the pinned lintian recipe
    /// records this hint without a pointer, although the Check passes
    /// `config` as one. Udebs are checked too.
    DebconfConfigNotExecutable,
    code = "LN0886",
    tag = "debconf-config-not-executable",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DebconfConfigNotExecutable {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let not_executable = {
            let Some(tree) = ctx.control_files() else {
                return;
            };
            tree.get("config").is_some_and(|entry| {
                matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink)
                    && entry.mode() & 0o111 == 0
            })
        };
        if not_executable {
            ctx.diagnostic(Diagnostic::error(""));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn config(file: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).control_file("config", file)
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            config(TestFile::new("#!/bin/sh\n").mode(0o755)),
            config(TestFile::new("#!/bin/sh\n").mode(0o700)),
            config(TestFile::symlink("missing")),
            config(TestFile::directory()),
        ];
        let fail = vec![
            config(TestFile::new("#!/bin/sh\n").mode(0o644)),
            config(TestFile::new("#!/bin/sh\n").mode(0o600)).udeb(),
        ];
        BinaryPackageTester::new(DebconfConfigNotExecutable, pass, fail).test_and_snapshot();
    }
}
