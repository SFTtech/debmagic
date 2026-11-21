//! Rules for lintian's `fonts/truetype` Check.

mod ln0708_truetype_font_prohibits_installable_embedding;
mod ln1092_truetype_font_wrong_filename;

pub(crate) use ln0708_truetype_font_prohibits_installable_embedding::TruetypeFontProhibitsInstallableEmbedding;
pub(crate) use ln1092_truetype_font_wrong_filename::TruetypeFontWrongFilename;
