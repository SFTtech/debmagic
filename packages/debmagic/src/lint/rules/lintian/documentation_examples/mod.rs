//! Rules for lintian's `documentation/examples` Check.
//!
//! Installed directories under `usr/share/doc/`.

mod ln0826_nested_examples_directory;

pub(crate) use ln0826_nested_examples_directory::NestedExamplesDirectory;
