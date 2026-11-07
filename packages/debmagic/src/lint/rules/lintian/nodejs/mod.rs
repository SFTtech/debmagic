//! Rules for lintian's `languages/javascript/nodejs` Check.
//!
//! `package.json` bytes under a nodejs module directory.

mod ln0425_nodejs_missing_version_override;

pub(crate) use ln0425_nodejs_missing_version_override::NodejsMissingVersionOverride;
