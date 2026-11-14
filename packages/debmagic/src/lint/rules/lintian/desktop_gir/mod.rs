//! Rules for lintian's `desktop/gnome/gir` Check.
//!
//! Each Rule owns one Tag. GIR XML files are the direct children of
//! `usr/share/gir-1.0` and `usr/lib/<triplet>/gir-1.0`.

mod ln0435_gir_section_not_libdevel;

pub(crate) use ln0435_gir_section_not_libdevel::GirSectionNotLibdevel;
