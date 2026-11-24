//! Rules for lintian's `languages/rust` Check.

mod ln1122_empty_rust_library_declares_provides;
mod ln1407_rust_boilerplate;

pub(crate) use ln1122_empty_rust_library_declares_provides::EmptyRustLibraryDeclaresProvides;
pub(crate) use ln1407_rust_boilerplate::RustBoilerplate;
