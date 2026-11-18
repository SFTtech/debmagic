//! Rules for lintian's `languages/ocaml/byte-code` Checks.
//!
//! Each Rule owns one Tag. `sibling` finds the file that shares an OCaml
//! artifact's name.

mod ln0816_ocaml_dangling_cmxa;
mod sibling;

pub(crate) use ln0816_ocaml_dangling_cmxa::OcamlDanglingCmxa;
