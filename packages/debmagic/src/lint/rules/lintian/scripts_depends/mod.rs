//! Maintainer scripts that need a dependency, call init scripts, or parse passwd.
//!
//! `script` walks maintainer-script text. `depends` is the shared package-name
//! check. Each Rule owns one Tag.

mod depends;
mod ln0194_maintainer_script_needs_depends_on_gconf2;
mod ln0195_maintainer_script_needs_depends_on_ucf;
mod ln0196_maintainer_script_needs_depends_on_xml_core;
mod ln0197_maintainer_script_needs_depends_on_xfonts_utils;
mod ln0198_maintainer_script_calls_init_script_directly;
mod ln0199_maintainer_script_should_not_parse_etc_passwd_or_group;
mod script;

pub(crate) use ln0194_maintainer_script_needs_depends_on_gconf2::MaintainerScriptNeedsDependsOnGconf2;
pub(crate) use ln0195_maintainer_script_needs_depends_on_ucf::MaintainerScriptNeedsDependsOnUcf;
pub(crate) use ln0196_maintainer_script_needs_depends_on_xml_core::MaintainerScriptNeedsDependsOnXmlCore;
pub(crate) use ln0197_maintainer_script_needs_depends_on_xfonts_utils::MaintainerScriptNeedsDependsOnXfontsUtils;
pub(crate) use ln0198_maintainer_script_calls_init_script_directly::MaintainerScriptCallsInitScriptDirectly;
pub(crate) use ln0199_maintainer_script_should_not_parse_etc_passwd_or_group::MaintainerScriptShouldNotParseEtcPasswdOrGroup;
