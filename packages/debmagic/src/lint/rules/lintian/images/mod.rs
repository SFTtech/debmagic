//! Rules for lintian's `images` Check.

mod ln1724_image_file_in_usr_lib;

pub(crate) use ln1724_image_file_in_usr_lib::ImageFileInUsrLib;
