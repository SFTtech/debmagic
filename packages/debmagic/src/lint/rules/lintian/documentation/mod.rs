//! Rules for lintian's `documentation` Check.

mod ln0713_zero_byte_file_in_doc_directory;
mod ln0825_executable_in_usr_share_doc;

pub(crate) use ln0713_zero_byte_file_in_doc_directory::ZeroByteFileInDocDirectory;
pub(crate) use ln0825_executable_in_usr_share_doc::ExecutableInUsrShareDoc;
