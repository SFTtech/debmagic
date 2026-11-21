use std::collections::BTreeMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct PythonModuleInWrongLocation;

declare_rule! {
    /// A Python module is installed under `usr/lib/python3[.N]/{site,dist}-packages/`
    /// where it should be under `usr/lib/python3/dist-packages/`.
    ///
    /// Each module (the first path component below the packages directory) is
    /// reported once as `actual -> correct`, for example
    /// `usr/lib/python3.2/site-packages/foo -> usr/lib/python3/dist-packages/foo`.
    /// The hint has no Location. Only paths under `usr/lib/python3` are checked.
    PythonModuleInWrongLocation,
    code = "LN0956",
    tag = "python-module-in-wrong-location",
    default_selected = true,
    experimental = false,
}

static PYTHON3_PACKAGES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^usr/lib/python(3(?:\.\d+)?)/((?:site|dist)-packages)/(.+)$").expect("valid regex")
});

impl BinaryPackageRule for PythonModuleInWrongLocation {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        // Sorted and de-duplicated by actual path, like lintian's hash of hints.
        let mut correct: BTreeMap<String, String> = BTreeMap::new();
        for entry in files.entries() {
            let Some(name) = entry.path().to_str() else {
                continue;
            };
            let Some(caps) = PYTHON3_PACKAGES.captures(name) else {
                continue;
            };
            let pyver = &caps[1];
            let actual_package_dir = &caps[2];
            let relative = &caps[3];
            let major = pyver.split('.').next().unwrap_or(pyver);
            let module_name = relative.split('/').next().unwrap_or(relative);

            let actual = format!("usr/lib/python{pyver}/{actual_package_dir}/{module_name}");
            let specified = format!("usr/lib/python{major}/dist-packages/{module_name}");
            if actual != specified {
                correct.insert(actual, specified);
            }
        }
        for (actual, specified) in correct {
            ctx.diagnostic(Diagnostic::warn(format!("{actual} -> {specified}")));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    #[test]
    fn binary() {
        let pass = vec![BinaryPackageCase::from(vec![
            (
                "usr/lib/python3/dist-packages/python-foo/__init__.py".to_string(),
                TestFile::new("x"),
            ),
            (
                "usr/lib/python3/dist-packages/python-foo".to_string(),
                TestFile::directory(),
            ),
            (
                "usr/lib/python2.7/dist-packages/bar.py".to_string(),
                TestFile::new("x"),
            ),
        ])];
        let fail = vec![
            // Site-packages is never correct, and a minor version in dist-packages is not either.
            BinaryPackageCase::from(vec![
                (
                    "usr/lib/python3.2/site-packages/python-foo".to_string(),
                    TestFile::directory(),
                ),
                (
                    "usr/lib/python3.2/site-packages/python-foo/__init__.py".to_string(),
                    TestFile::new("x"),
                ),
                (
                    "usr/lib/python3.2/dist-packages/python-foo".to_string(),
                    TestFile::directory(),
                ),
            ]),
            // Udeb packages are checked as well.
            BinaryPackageCase::from(vec![(
                "usr/lib/python3.2/site-packages/foo.py".to_string(),
                TestFile::new("x"),
            )])
            .udeb(),
        ];
        BinaryPackageTester::new(PythonModuleInWrongLocation, pass, fail).test_and_snapshot();
    }
}
