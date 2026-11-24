//! Rules for lintian's `fonts/opentype` Check.

mod ln0736_opentype_font_prohibits_installable_embedding;
mod ln0784_opentype_font_wrong_filename;

pub(crate) use ln0736_opentype_font_prohibits_installable_embedding::OpentypeFontProhibitsInstallableEmbedding;
pub(crate) use ln0784_opentype_font_wrong_filename::OpentypeFontWrongFilename;
