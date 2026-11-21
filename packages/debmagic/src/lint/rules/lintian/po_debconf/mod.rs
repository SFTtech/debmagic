//! Rules for lintian's `debian/po-debconf` Check.
//!
//! `files` finds template files and `debian/po` translations. Each Rule owns
//! one Tag.

mod files;
mod ln0381_misnamed_po_file;
mod ln0382_translated_default_field;
mod ln0383_invalid_po_file;
mod ln0902_debconf_translation_using_general_list;
mod ln0904_missing_file_from_potfiles_in;
mod ln0905_missing_potfiles_in;
mod ln0906_missing_templates_pot;
mod ln0908_not_using_po_debconf;

pub(crate) use ln0381_misnamed_po_file::MisnamedPoFile;
pub(crate) use ln0382_translated_default_field::TranslatedDefaultField;
pub(crate) use ln0383_invalid_po_file::InvalidPoFile;
pub(crate) use ln0902_debconf_translation_using_general_list::DebconfTranslationUsingGeneralList;
pub(crate) use ln0904_missing_file_from_potfiles_in::MissingFileFromPotfilesIn;
pub(crate) use ln0905_missing_potfiles_in::MissingPotfilesIn;
pub(crate) use ln0906_missing_templates_pot::MissingTemplatesPot;
pub(crate) use ln0908_not_using_po_debconf::NotUsingPoDebconf;
