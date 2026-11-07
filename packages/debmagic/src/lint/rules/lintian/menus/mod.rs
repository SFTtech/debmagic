//! Menu, menu-method, and doc-base Rules from lintian's `menus` Check.
//!
//! Each Rule owns one Tag. `doc-base-file-references-missing-file` is absent:
//! lintian treats a path as present when a strongly related installable ships
//! it, and that needs other binary packages.

mod ln0326_postinst_does_not_call_updatemenus;
mod ln0327_postrm_does_not_call_updatemenus;
mod ln0328_postinst_has_useless_call_to_install_docs;
mod ln0329_prerm_has_useless_call_to_install_docs;
mod ln0331_doc_base_abstract_field_separator_extra_whitespace;
mod ln0332_doc_base_file_no_format_section;
mod ln0333_menu_method_lacks_include;
mod ln0420_doc_base_file_lacks_required_field;
mod scan;

pub(crate) use ln0326_postinst_does_not_call_updatemenus::PostinstDoesNotCallUpdatemenus;
pub(crate) use ln0327_postrm_does_not_call_updatemenus::PostrmDoesNotCallUpdatemenus;
pub(crate) use ln0328_postinst_has_useless_call_to_install_docs::PostinstHasUselessCallToInstallDocs;
pub(crate) use ln0329_prerm_has_useless_call_to_install_docs::PrermHasUselessCallToInstallDocs;
pub(crate) use ln0331_doc_base_abstract_field_separator_extra_whitespace::DocBaseAbstractFieldSeparatorExtraWhitespace;
pub(crate) use ln0332_doc_base_file_no_format_section::DocBaseFileNoFormatSection;
pub(crate) use ln0333_menu_method_lacks_include::MenuMethodLacksInclude;
pub(crate) use ln0420_doc_base_file_lacks_required_field::DocBaseFileLacksRequiredField;
