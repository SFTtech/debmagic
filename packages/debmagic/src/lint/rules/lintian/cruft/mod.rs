//! Rules for lintian's `cruft` Check.

mod ln1267_license_problem_cc_by_nc_sa;
mod ln1269_license_problem_json_evil;

pub(crate) use ln1267_license_problem_cc_by_nc_sa::LicenseProblemCcByNcSa;
pub(crate) use ln1269_license_problem_json_evil::LicenseProblemJsonEvil;
