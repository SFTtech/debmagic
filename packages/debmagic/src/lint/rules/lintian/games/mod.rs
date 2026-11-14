//! `games`: section `games` together with `/usr/bin` or `/bin`.

mod ln0401_package_section_games_but_contains_no_game;
mod ln0402_package_section_games_but_has_usr_bin;
mod paths;

pub(crate) use ln0401_package_section_games_but_contains_no_game::PackageSectionGamesButContainsNoGame;
pub(crate) use ln0402_package_section_games_but_has_usr_bin::PackageSectionGamesButHasUsrBin;
