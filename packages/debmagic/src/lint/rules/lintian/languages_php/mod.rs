//! Rules for lintian's `languages/php/embedded` Check.

mod ln0652_embedded_php_library;
mod ln0820_obsolete_comments_style_in_php_ini;
mod ln1112_composer_prerequisite;
mod ln1282_missing_pkg_php_tools_addon;
mod ln1283_missing_pkg_php_tools_buildsystem;
mod ln1284_pear_channel_without_pkg_php_tools_builddep;
mod ln1285_pear_package_but_missing_dependency;
mod pear;

pub(crate) use ln0652_embedded_php_library::EmbeddedPhpLibrary;
pub(crate) use ln0820_obsolete_comments_style_in_php_ini::ObsoleteCommentsStyleInPhpIni;
pub(crate) use ln1112_composer_prerequisite::ComposerPrerequisite;
pub(crate) use ln1282_missing_pkg_php_tools_addon::MissingPkgPhpToolsAddon;
pub(crate) use ln1283_missing_pkg_php_tools_buildsystem::MissingPkgPhpToolsBuildsystem;
pub(crate) use ln1284_pear_channel_without_pkg_php_tools_builddep::PearChannelWithoutPkgPhpToolsBuilddep;
pub(crate) use ln1285_pear_package_but_missing_dependency::PearPackageButMissingDependency;
