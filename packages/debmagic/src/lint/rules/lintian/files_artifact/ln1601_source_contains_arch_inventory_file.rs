use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::SourceRule;

use super::added::{Artifact, flag_native_pedantic};

pub struct SourceContainsArchInventoryFile;

declare_rule! {
    /// A native package contains an Arch inventory file.  A regular file or
    /// hardlink named `.arch-inventory`, at any depth.  Only for a native package
    /// (`3.0 (native)`, or `1.0` without an orig tarball). Pedantic, as in lintian.
    /// Extra is empty; the path is the Location. The non-native case is `debian-
    /// adds-arch-inventory-file`.
    SourceContainsArchInventoryFile,
    code = "LN1601",
    tag = "source-contains-arch-inventory-file",
    default_selected = false,
    experimental = false,
}

impl SourceRule for SourceContainsArchInventoryFile {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_native_pedantic(ctx, Artifact::ArchInventoryFile);
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageTester, SourceTreeTester};

    use super::super::testing::{changelog, dir, file, hardlink, native, native_tree, symlink};
    use super::*;

    #[test]
    fn test() {
        let pass = vec![
            native_tree(vec![dir(".arch-inventory")]),
            native_tree(vec![file("arch-inventory")]),
            native_tree(vec![symlink(".arch-inventory")]),
        ];
        let fail = vec![
            native_tree(vec![file(".arch-inventory")]),
            native_tree(vec![file("debian/.arch-inventory")]),
            native_tree(vec![hardlink(".arch-inventory")]),
        ];
        SourceTreeTester::new(SourceContainsArchInventoryFile, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            native(vec![changelog(), dir(".arch-inventory")]),
            native(vec![changelog(), file("arch-inventory")]),
            native(vec![changelog(), symlink(".arch-inventory")]),
        ];
        let fail = vec![
            native(vec![changelog(), file(".arch-inventory")]),
            native(vec![changelog(), file("debian/.arch-inventory")]),
            native(vec![changelog(), hardlink(".arch-inventory")]),
        ];
        SourcePackageTester::new(SourceContainsArchInventoryFile, pass, fail).test_and_snapshot();
    }
}
