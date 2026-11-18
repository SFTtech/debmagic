//! Rules for lintian's `modprobe` Check.
//!
//! Installed files under `etc` or `usr/lib` `modprobe.d` and `modules-load.d`.

mod ln0414_obsolete_command_in_modprobe_d_file;
mod ln0782_directory_in_modprobe_d;
mod scope;

pub(crate) use ln0414_obsolete_command_in_modprobe_d_file::ObsoleteCommandInModprobeDFile;
pub(crate) use ln0782_directory_in_modprobe_d::DirectoryInModprobeD;
