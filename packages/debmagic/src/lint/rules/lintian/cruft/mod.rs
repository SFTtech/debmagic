//! Rules for lintian's `cruft` Check.

mod ln1267_license_problem_cc_by_nc_sa;
mod ln1269_license_problem_json_evil;
mod ln1270_license_problem_non_free_rfc_bcp78;
mod ln1271_license_problem_non_free_rfc;
mod ln1272_license_problem_nvidia_intellectual;

pub(crate) use ln1267_license_problem_cc_by_nc_sa::LicenseProblemCcByNcSa;
pub(crate) use ln1269_license_problem_json_evil::LicenseProblemJsonEvil;
pub(crate) use ln1270_license_problem_non_free_rfc_bcp78::LicenseProblemNonFreeRFCBCP78;
pub(crate) use ln1271_license_problem_non_free_rfc::LicenseProblemNonFreeRFC;
pub(crate) use ln1272_license_problem_nvidia_intellectual::LicenseProblemNvidiaIntellectual;
