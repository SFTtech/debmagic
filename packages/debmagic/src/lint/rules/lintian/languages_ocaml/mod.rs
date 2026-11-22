//! Rules for lintian's `languages/ocaml/byte-code` Checks.
//!
//! Each Rule owns one Tag. `sibling` finds the file that shares an OCaml
//! artifact's name.

mod ln0816_ocaml_dangling_cmxa;
mod ln0817_ocaml_dangling_cmxs;
mod ln1400_ocaml_dangling_cmi;
mod sibling;

pub(crate) use ln0816_ocaml_dangling_cmxa::OcamlDanglingCmxa;
pub(crate) use ln0817_ocaml_dangling_cmxs::OcamlDanglingCmxs;
pub(crate) use ln1400_ocaml_dangling_cmi::OcamlDanglingCmi;
