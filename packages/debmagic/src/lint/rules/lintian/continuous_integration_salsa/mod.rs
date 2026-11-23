//! Rules for lintian's `continuous-integration/salsa` Check.

mod ln1788_invalid_yaml;

pub(crate) use ln1788_invalid_yaml::InvalidYaml;
