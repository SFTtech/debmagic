//! Rules for lintian's `files/banned` Check.

mod ln1375_license_problem_md5sum_non_distributable_file;
mod ln1376_license_problem_non_free_img_lenna;

pub(crate) use ln1375_license_problem_md5sum_non_distributable_file::LicenseProblemMd5sumNonDistributableFile;
pub(crate) use ln1376_license_problem_non_free_img_lenna::LicenseProblemNonFreeImgLenna;
