//! Rules for lintian's `debian/not-installed` Check.

mod ln1028_unwanted_path_too_specific;

pub(crate) use ln1028_unwanted_path_too_specific::UnwantedPathTooSpecific;
