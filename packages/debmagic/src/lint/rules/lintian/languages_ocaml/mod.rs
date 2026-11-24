//! Rules for lintian's `languages/ocaml/byte-code` Checks.
//!
//! Each Rule owns one Tag. `sibling` finds the file that shares an OCaml
//! artifact's name.

mod dev_files;
mod ln0816_ocaml_dangling_cmxa;
mod ln0817_ocaml_dangling_cmxs;
mod ln1399_ocaml_dangling_cmx;
mod ln1400_ocaml_dangling_cmi;
mod ln1401_ocaml_custom_executable;
mod ln1747_ocaml_dev_file_in_nondev_package;
mod ln1748_ocaml_dev_file_not_in_usr_lib_ocaml;
mod ln1749_ocaml_meta_without_suggesting_findlib;
mod ln1750_ocaml_stray_cmo;
mod sibling;

pub(crate) use ln0816_ocaml_dangling_cmxa::OcamlDanglingCmxa;
pub(crate) use ln0817_ocaml_dangling_cmxs::OcamlDanglingCmxs;
pub(crate) use ln1399_ocaml_dangling_cmx::OcamlDanglingCmx;
pub(crate) use ln1400_ocaml_dangling_cmi::OcamlDanglingCmi;
pub(crate) use ln1401_ocaml_custom_executable::OcamlCustomExecutable;
pub(crate) use ln1747_ocaml_dev_file_in_nondev_package::OcamlDevFileInNondevPackage;
pub(crate) use ln1748_ocaml_dev_file_not_in_usr_lib_ocaml::OcamlDevFileNotInUsrLibOcaml;
pub(crate) use ln1749_ocaml_meta_without_suggesting_findlib::OcamlMetaWithoutSuggestingFindlib;
pub(crate) use ln1750_ocaml_stray_cmo::OcamlStrayCmo;
