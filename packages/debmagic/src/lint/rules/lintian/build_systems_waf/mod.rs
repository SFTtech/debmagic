//! Rules for lintian's `build-systems/waf` Check.

mod ln1011_source_contains_waf_binary;

pub(crate) use ln1011_source_contains_waf_binary::SourceContainsWafBinary;
