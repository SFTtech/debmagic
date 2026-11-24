//! Rules for lintian's `languages/ruby` Check.

mod ln1120_ruby_interpreter_is_deprecated;
mod ln1121_rubygem_homepage;

pub(crate) use ln1120_ruby_interpreter_is_deprecated::RubyInterpreterIsDeprecated;
pub(crate) use ln1121_rubygem_homepage::RubygemHomepage;
