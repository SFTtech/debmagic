//! Rules for lintian's `debian/po-debconf` Check.
//!
//! `files` finds template files and `debian/po` translations. Each Rule owns
//! one Tag.

mod files;
mod ln0381_misnamed_po_file;
mod ln0382_translated_default_field;
mod ln0383_invalid_po_file;

pub(crate) use ln0381_misnamed_po_file::MisnamedPoFile;
pub(crate) use ln0382_translated_default_field::TranslatedDefaultField;
pub(crate) use ln0383_invalid_po_file::InvalidPoFile;
