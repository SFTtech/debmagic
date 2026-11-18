//! Rules for lintian's `languages/php/embedded` Check.

mod ln0652_embedded_php_library;
mod ln0820_obsolete_comments_style_in_php_ini;

pub(crate) use ln0652_embedded_php_library::EmbeddedPhpLibrary;
pub(crate) use ln0820_obsolete_comments_style_in_php_ini::ObsoleteCommentsStyleInPhpIni;
