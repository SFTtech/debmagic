//! Rules for lintian's `debian/control/link` Check.

mod ln1019_debian_control_file_is_a_symlink;

pub(crate) use ln1019_debian_control_file_is_a_symlink::DebianControlFileIsASymlink;
