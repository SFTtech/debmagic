use std::path::PathBuf;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::ln1102_gfortran_module_does_not_declare_version::{at, gfortran_modules};

pub struct MissingPrerequisiteForGfortranModule;

declare_rule! {
    /// A gzip-compressed `.mod` file under `usr/lib/` declares GFORTRAN module
    /// version `<n>`, but the installation package's `Depends` does not name
    /// `gfortran-mod-<n>` as a whole word.
    ///
    /// The module files and their version are found as
    /// `gfortran-module-does-not-declare-version` (LN1102) finds them; a module
    /// that declares no version is that Tag's, not this one. `Depends` is read
    /// unfolded from the installation control, and a missing field counts as
    /// empty. Extra is empty; the module path is the Location.
    MissingPrerequisiteForGfortranModule,
    code = "LN1103",
    tag = "missing-prerequisite-for-gfortran-module",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MissingPrerequisiteForGfortranModule {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let versioned: Vec<(PathBuf, String)> = gfortran_modules(files)
            .into_iter()
            .filter_map(|module| module.version.map(|version| (module.path, version)))
            .collect();
        if versioned.is_empty() {
            return;
        }
        let depends = ctx
            .installation_control()
            .fields()
            .unfolded_value("Depends")
            .unwrap_or_default();
        for (path, version) in versioned {
            let prerequisite =
                Regex::new(&format!(r"\bgfortran-mod-{version}\b")).expect("a version is digits");
            if !prerequisite.is_match(&depends) {
                ctx.diagnostic(Diagnostic::warn("").with_location(at(&path)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn gzip(text: &str) -> TestFile {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(text.as_bytes()).expect("gzip");
        TestFile::from_bytes(encoder.finish().expect("gzip"))
    }

    fn module(path: &str, version: &str) -> BinaryPackageCase {
        let text = format!("GFORTRAN module version '{version}' created from mpi.f90\n");
        BinaryPackageCase::from(vec![(path.to_string(), gzip(&text))])
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            module("usr/lib/fortran/mpi.mod", "14").field("Depends", "libc6, gfortran-mod-14"),
            module("usr/lib/fortran/mpi.mod", "14")
                .field("Depends", "libc6,\n gfortran-mod-14 (>= 14.1)"),
            // Only `Depends` counts, not `Pre-Depends`.
            // Without a version the other Tag applies.
            BinaryPackageCase::from(vec![("usr/lib/fortran/mpi.mod", gzip("no version\n"))]),
            // Not gzip-compressed, outside `usr/lib/`, or in a skipped subtree.
            BinaryPackageCase::from(vec![(
                "usr/lib/fortran/mpi.mod",
                TestFile::new("GFORTRAN module version '14'\n"),
            )]),
            module("usr/share/fortran/mpi.mod", "14"),
            module("usr/lib/flang-14/mpi.mod", "14"),
            module("usr/lib/grub/mpi.mod", "14"),
            module("usr/lib/libreoffice/mpi.mod", "14"),
        ];
        let fail = vec![
            module("usr/lib/fortran/mpi.mod", "14"),
            module("usr/lib/fortran/mpi.mod", "14").field("Depends", "libc6"),
            module("usr/lib/fortran/mpi.mod", "14").field("Depends", "gfortran-mod-13"),
            // The prerequisite is matched as a whole word.
            module("usr/lib/fortran/mpi.mod", "14").field("Depends", "gfortran-mod-140"),
            module("usr/lib/fortran/mpi.mod", "14").field("Depends", "gfortran-mod-1"),
            // Only `Depends` counts, not `Pre-Depends`.
            module("usr/lib/fortran/mpi.mod", "14")
                .field("Pre-Depends", "gfortran-mod-14")
                .field("Depends", "libc6"),
            // Every module is judged on its own.
            BinaryPackageCase::from(vec![
                (
                    "usr/lib/fortran/mpi.mod",
                    gzip("GFORTRAN module version '14'\n"),
                ),
                (
                    "usr/lib/fortran/other.mod",
                    gzip("GFORTRAN module version '13'\n"),
                ),
            ])
            .field("Depends", "gfortran-mod-14"),
            // Udebs are checked too.
            module("usr/lib/fortran/mpi.mod", "14").udeb(),
        ];
        BinaryPackageTester::new(MissingPrerequisiteForGfortranModule, pass, fail)
            .test_and_snapshot();
    }
}
