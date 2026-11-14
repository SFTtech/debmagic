use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::paths::game_bins;

pub struct PackageSectionGamesButHasUsrBin;

declare_rule! {
    /// Section ends in `games` and the package has both `usr/games` and
    /// `bin` or `usr/bin` children.
    ///
    /// A direct child counts, of any kind. Extra is empty. Udebs are checked.
    PackageSectionGamesButHasUsrBin,
    code = "LN0402",
    tag = "package-section-games-but-has-usr-bin",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PackageSectionGamesButHasUsrBin {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let section = ctx
            .installation_control()
            .fields()
            .unfolded_value("Section")
            .unwrap_or_default();
        if !section.ends_with("games") {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let bins = game_bins(files);
        if bins.other > 0 && bins.games > 0 {
            ctx.diagnostic(Diagnostic::warn(""));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn packaged(section: &str, paths: &[&str]) -> BinaryPackageCase {
        let files = paths
            .iter()
            .map(|path| ((*path).to_string(), TestFile::new("x")))
            .collect::<Vec<_>>();
        BinaryPackageCase::from(files).field("Section", section)
    }

    #[test]
    fn test() {
        let pass = vec![
            packaged("games", &["usr/games/blaster"]),
            packaged("games", &["usr/bin/tool"]),
            packaged("utils", &["usr/bin/tool", "usr/games/blaster"]),
        ];
        let fail = vec![
            packaged("games", &["usr/bin/tool", "usr/games/blaster"]),
            packaged("contrib/games", &["bin/tool", "usr/games/blaster"]).udeb(),
        ];
        BinaryPackageTester::new(PackageSectionGamesButHasUsrBin, pass, fail).test_and_snapshot();
    }
}
