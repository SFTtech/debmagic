//! Menu files and desktop entries, matching lintian's `menu-format` Check.
//!
//! Each Rule owns one Tag. Menu files are the direct children of
//! `usr/share/menu` and `usr/lib/menu`. Desktop files are `*.desktop` directly
//! under `usr/share/applications` and `usr/share/xsessions`. Bytes come from
//! [`FileTree::read`](crate::lint::file_tree::FileTree::read).

mod data;
mod ln0248_menu_command_not_in_package;
mod ln0249_command_in_menu_file_and_desktop_file;
mod ln0250_menu_item_creates_new_section;
mod ln0251_desktop_command_not_in_package;
mod ln0252_desktop_entry_contains_unknown_key;
mod ln0253_menu_item_uses_apps_section;
mod ln0254_menu_item_uses_windowmanagers_section;
mod ln0255_non_wm_in_windowmanager_menu_section;
mod ln0256_desktop_entry_file_has_crs;
mod ln0257_desktop_entry_invalid_category;
mod ln0258_desktop_entry_uses_reserved_category;
mod ln0259_pkg_not_in_package_test;
#[allow(non_snake_case)]
mod ln0417_su_wrapper_without__c;
mod ln0418_unquoted_string_in_menu_item;
mod ln0423_menu_icon_uses_relative_path;
mod ln0429_su_wrapper_not_su_to_root;
mod scan;

pub(crate) use ln0248_menu_command_not_in_package::MenuCommandNotInPackage;
pub(crate) use ln0249_command_in_menu_file_and_desktop_file::CommandInMenuFileAndDesktopFile;
pub(crate) use ln0250_menu_item_creates_new_section::MenuItemCreatesNewSection;
pub(crate) use ln0251_desktop_command_not_in_package::DesktopCommandNotInPackage;
pub(crate) use ln0252_desktop_entry_contains_unknown_key::DesktopEntryContainsUnknownKey;
pub(crate) use ln0253_menu_item_uses_apps_section::MenuItemUsesAppsSection;
pub(crate) use ln0254_menu_item_uses_windowmanagers_section::MenuItemUsesWindowmanagersSection;
pub(crate) use ln0255_non_wm_in_windowmanager_menu_section::NonWmInWindowmanagerMenuSection;
pub(crate) use ln0256_desktop_entry_file_has_crs::DesktopEntryFileHasCrs;
pub(crate) use ln0257_desktop_entry_invalid_category::DesktopEntryInvalidCategory;
pub(crate) use ln0258_desktop_entry_uses_reserved_category::DesktopEntryUsesReservedCategory;
pub(crate) use ln0259_pkg_not_in_package_test::PkgNotInPackageTest;
pub(crate) use ln0417_su_wrapper_without__c::SuWrapperWithoutC;
pub(crate) use ln0418_unquoted_string_in_menu_item::UnquotedStringInMenuItem;
pub(crate) use ln0423_menu_icon_uses_relative_path::MenuIconUsesRelativePath;
pub(crate) use ln0429_su_wrapper_not_su_to_root::SuWrapperNotSuToRoot;
