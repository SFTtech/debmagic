//! Rules for lintian's `debian/variables` Check.
//!
//! Each Rule reads `debian/` install and links files from the Source tree or
//! Source package Patched files. Each Rule emits one Tag.

mod ln0369_illegal_variable;

pub(crate) use ln0369_illegal_variable::IllegalVariable;
