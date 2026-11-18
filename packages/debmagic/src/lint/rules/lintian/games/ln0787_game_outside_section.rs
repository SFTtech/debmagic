use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::paths::game_bins;

pub struct GameOutsideSection;

declare_rule! {
    /// `usr/games` has a direct child, `bin` and `usr/bin` do not, and the
    /// section does not end in `games`.
    ///
    /// A missing Section counts as not ending in `games`. Any direct child of
    /// `bin` or `usr/bin` suppresses the Tag, even when `usr/games` is also
    /// populated. Extra is empty. Udebs are checked.
    GameOutsideSection,
    code = "LN0787",
    tag = "game-outside-section",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for GameOutsideSection {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let section = ctx
            .installation_control()
            .fields()
            .unfolded_value("Section")
            .unwrap_or_default();
        if section.ends_with("games") {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let bins = game_bins(files);
        if bins.other == 0 && bins.games > 0 {
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
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            packaged("games", &["usr/games/blaster"]),
            packaged("non-free/games", &["usr/games/only"]),
            // Binaries in `usr/bin` suppress the Tag.
            packaged("utils", &["usr/bin/blaster", "usr/games/blaster"]),
            packaged("utils", &["bin/tool"]),
            // `usr/games` with no direct child does not count.
            packaged("utils", &["usr/games"]),
            packaged("utils", &["usr/share/games/data"]),
            packaged("utils", &["usr/bin/tool"]),
        ];
        let fail = vec![
            packaged("utils", &["usr/games/blaster"]),
            packaged("non-free/utils", &["usr/games/blaster", "usr/share/doc/x"]),
            packaged("", &["usr/games/blaster"]),
            packaged("utils", &["usr/games/blaster"]).udeb(),
        ];
        BinaryPackageTester::new(GameOutsideSection, pass, fail).test_and_snapshot();
    }
}
