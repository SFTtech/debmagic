//! Rules for lintian's `languages/javascript/nodejs` Check.
//!
//! Each Rule owns one Tag. Installed paths are matched on their own.
//! Directories are ignored, and packages whose names end in `-dbg` are not
//! checked.

mod detect;
mod ln0403_pkg_js_tools_test_is_empty;
mod ln0404_pkg_js_tools_test_is_missing;

pub(crate) use ln0403_pkg_js_tools_test_is_empty::PkgJsToolsTestIsEmpty;
pub(crate) use ln0404_pkg_js_tools_test_is_missing::PkgJsToolsTestIsMissing;
