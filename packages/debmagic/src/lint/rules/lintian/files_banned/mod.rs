//! Rules for lintian's `files/banned` Check.

mod ln1498_license_problem_md5sum_non_distributable_file;
mod ln1499_license_problem_non_free_img_lenna;

pub(crate) use ln1498_license_problem_md5sum_non_distributable_file::LicenseProblemMd5sumNonDistributableFile;
pub(crate) use ln1499_license_problem_non_free_img_lenna::LicenseProblemNonFreeImgLenna;
