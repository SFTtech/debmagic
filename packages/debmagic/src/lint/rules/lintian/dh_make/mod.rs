//! Rules for lintian's `dh-make` Check.

mod ln1046_readme_source_is_dh_make_template;
mod ln1278_dh_make_template_in_source;

pub(crate) use ln1046_readme_source_is_dh_make_template::ReadmeSourceIsDhMakeTemplate;
pub(crate) use ln1278_dh_make_template_in_source::DhMakeTemplateInSource;
