//! Rules for lintian's `modprobe` Check.
//!
//! Installed files under `etc` or `usr/lib` `modprobe.d` and `modules-load.d`.

mod ln0414_obsolete_command_in_modprobe_d_file;

pub(crate) use ln0414_obsolete_command_in_modprobe_d_file::ObsoleteCommandInModprobeDFile;
