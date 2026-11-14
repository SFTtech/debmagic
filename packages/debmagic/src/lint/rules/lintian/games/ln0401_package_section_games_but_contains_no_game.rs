use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::paths::game_bins;

pub struct PackageSectionGamesButContainsNoGame;

declare_rule! {
    /// Section ends in `games`, binaries live in `bin` or `usr/bin`, and
    /// `usr/games` is empty.
    ///
    /// A direct child of those directories counts, of any kind. Nothing in
    /// `bin` and `usr/bin` does not emit, even when `usr/games` is also empty.
    /// Extra is empty. Udebs are checked.
    PackageSectionGamesButContainsNoGame,
    code = "LN0401",
    tag = "package-section-games-but-contains-no-game",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PackageSectionGamesButContainsNoGame {
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
        if bins.other > 0 && bins.games == 0 {
            ctx.diagnostic(Diagnostic::error(""));
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
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()).field("Section", "games"),
            packaged("games", &["usr/games/blaster"]),
            packaged("games", &["usr/bin/blaster", "usr/games/blaster"]),
            packaged("utils", &["usr/bin/tool"]),
            packaged("non-free/games", &["usr/games/only"]),
        ];
        let fail = vec![
            packaged("games", &["usr/bin/tool"]),
            packaged("non-free/games", &["bin/tool"]),
            packaged("games", &["usr/bin/tool", "usr/bin/other"]).udeb(),
        ];
        BinaryPackageTester::new(PackageSectionGamesButContainsNoGame, pass, fail)
            .test_and_snapshot();
    }
}
