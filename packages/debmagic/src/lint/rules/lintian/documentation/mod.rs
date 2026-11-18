//! Rules for lintian's `documentation` Check.

mod ln0713_zero_byte_file_in_doc_directory;

pub(crate) use ln0713_zero_byte_file_in_doc_directory::ZeroByteFileInDocDirectory;
