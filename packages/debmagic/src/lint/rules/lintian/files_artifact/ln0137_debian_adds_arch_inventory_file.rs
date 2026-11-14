use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::rule::SourceRule;

use super::added::{Artifact, flag_added};

pub struct DebianAddsArchInventoryFile;

declare_rule! {
    /// Debian added an Arch inventory file (`.arch-inventory`).
    ///
    /// Regular files and hardlinks. The name must be the whole final
    /// component. Extra is empty; the path is the Location.
    DebianAddsArchInventoryFile,
    code = "LN0137",
    tag = "debian-adds-arch-inventory-file",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DebianAddsArchInventoryFile {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        flag_added(ctx, Artifact::ArchInventoryFile);
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageTester, SourceTreeTester};

    use super::super::testing::{
        added, control, dir, file, hardlink, in_orig, native, quilt, symlink,
    };
    use super::*;

    #[test]
    fn test() {
        let pass = vec![
            vec![control(), file("arch-inventory")],
            vec![control(), file(".arch-inventory.bak")],
            vec![control(), file("foo.arch-inventory")],
            vec![control(), dir(".arch-inventory")],
            vec![control(), symlink(".arch-inventory")],
        ];
        let fail = vec![
            vec![control(), file(".arch-inventory")],
            vec![control(), file("debian/.arch-inventory")],
            vec![control(), hardlink("src/.arch-inventory")],
            vec![control(), file(".pc/.arch-inventory")],
        ];
        SourceTreeTester::new(DebianAddsArchInventoryFile, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            native(vec![file(".arch-inventory")]),
            in_orig(vec![file(".arch-inventory")]),
            quilt(vec![file(".pc/.arch-inventory")]),
            added(vec![file("arch-inventory")]),
            added(vec![dir(".arch-inventory")]),
        ];
        let fail = vec![
            added(vec![file("debian/.arch-inventory")]),
            added(vec![file(".arch-inventory")]),
            added(vec![file(".pc/.arch-inventory")]),
        ];
        SourcePackageTester::new(DebianAddsArchInventoryFile, pass, fail).test_and_snapshot();
    }
}
