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
mod ln0443_menu_item_needs_tag_has_unknown_value;
mod ln0505_menu_icon_missing;
mod ln0517_menu_item_missing_required_tag;
mod ln0620_non_wm_module_in_wm_modules_menu_section;
mod ln0621_su_to_root_with_usr_sbin;
mod ln0622_bad_test_in_menu_item;
mod ln0623_duplicate_tag_in_menu;
mod ln0624_desktop_contains_deprecated_key;
mod ln0625_desktop_entry_contains_deprecated_key;
mod ln0626_duplicate_key_in_desktop;
mod ln0627_desktop_entry_unknown_type;
mod ln0628_desktop_entry_missing_required_key;
mod ln0629_desktop_entry_lacks_main_category;
mod ln0630_desktop_mime_but_no_exec_code;
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
pub(crate) use ln0443_menu_item_needs_tag_has_unknown_value::MenuItemNeedsTagHasUnknownValue;
pub(crate) use ln0505_menu_icon_missing::MenuIconMissing;
pub(crate) use ln0517_menu_item_missing_required_tag::MenuItemMissingRequiredTag;
pub(crate) use ln0620_non_wm_module_in_wm_modules_menu_section::NonWmModuleInWmModulesMenuSection;
pub(crate) use ln0621_su_to_root_with_usr_sbin::SuToRootWithUsrSbin;
pub(crate) use ln0622_bad_test_in_menu_item::BadTestInMenuItem;
pub(crate) use ln0623_duplicate_tag_in_menu::DuplicateTagInMenu;
pub(crate) use ln0624_desktop_contains_deprecated_key::DesktopContainsDeprecatedKey;
pub(crate) use ln0625_desktop_entry_contains_deprecated_key::DesktopEntryContainsDeprecatedKey;
pub(crate) use ln0626_duplicate_key_in_desktop::DuplicateKeyInDesktop;
pub(crate) use ln0627_desktop_entry_unknown_type::DesktopEntryUnknownType;
pub(crate) use ln0628_desktop_entry_missing_required_key::DesktopEntryMissingRequiredKey;
pub(crate) use ln0629_desktop_entry_lacks_main_category::DesktopEntryLacksMainCategory;
pub(crate) use ln0630_desktop_mime_but_no_exec_code::DesktopMimeButNoExecCode;
