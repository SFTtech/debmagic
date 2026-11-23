use crate::declare_rule;
use crate::lint::context::{SourceContext, follow_symlinks};
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

use super::ln1403_missing_prerequisite_for_pyproject_backend::build_backend;

pub struct UsesPdmCli;

declare_rule! {
    /// `pyproject.toml` names `pdm.pep517.api` as its `build-backend` and
    /// `Build-Depends` satisfies `python3-pdm` (the command-line tool) but not
    /// `python3-pdm-backend`, the prerequisite the backend needs. Reported at
    /// info severity, at the `build-backend` line, with an empty extra.
    ///
    /// `Build-Depends` alone is read, with its architecture, version and
    /// profile restrictions dropped, as lintian's `load_norestriction`. The
    /// `pyproject.toml` is followed through symlinks inside the tree and is
    /// skipped when it is not an open file. The sibling for poetry is
    /// LN1686 (`uses-poetry-cli`).
    UsesPdmCli,
    code = "LN1685",
    tag = "uses-pdm-cli",
    default_selected = false,
    experimental = false,
}

impl SourceRule for UsesPdmCli {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let build_depends = control
            .source()
            .relation("Build-Depends")
            .map(|relation| relation.restriction_less())
            .unwrap_or_default();
        if !build_depends.satisfies("python3-pdm") || build_depends.satisfies("python3-pdm-backend")
        {
            return;
        }
        let Some(files) = ctx.files() else {
            return;
        };
        let Some(entry) = files.get("pyproject.toml") else {
            return;
        };
        let Some(target) = follow_symlinks(files, entry) else {
            return;
        };
        let Ok(bytes) = files.read(target) else {
            return;
        };
        let path = target.path().to_path_buf();

        let lines: Vec<u32> = bytes
            .split(|byte| *byte == b'\n')
            .enumerate()
            .filter(|(_, line)| {
                build_backend(&String::from_utf8_lossy(line)) == Some("pdm.pep517.api")
            })
            .map(|(index, _)| index as u32 + 1)
            .collect();
        for line in lines {
            ctx.diagnostic(
                Diagnostic::warn("")
                    .with_severity(Severity::Info)
                    .with_location(Location {
                        path: path.clone(),
                        line: Some(line),
                        column: None,
                    }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn tree(build_depends: &str, pyproject: Option<&str>) -> Vec<(&'static str, TestFile)> {
        let mut files = vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: example\n\
                 Maintainer: Example <ex@example.com>\n\
                 Standards-Version: 4.7.2\n\
                 Build-Depends: {build_depends}\n\
                 \n\
                 Package: example\n\
                 Architecture: all\n\
                 Description: example\n extra\n"
            )),
        )];
        if let Some(pyproject) = pyproject {
            files.push(("pyproject.toml", TestFile::new(pyproject)));
        }
        files
    }

    const PDM: &str = "[build-system]\n\
                       requires = [\"pdm-backend\"]\n\
                       build-backend = \"pdm.pep517.api\"\n";

    const SETUPTOOLS: &str = "[build-system]\n\
                              build-backend = \"setuptools.build_meta\"\n";

    #[test]
    fn test() {
        let pass = vec![
            tree("debhelper-compat (= 14), python3-pdm-backend", Some(PDM)),
            tree(
                "debhelper-compat (= 14), python3-pdm, python3-pdm-backend",
                Some(PDM),
            ),
            tree("python3-pdm", Some(SETUPTOOLS)),
            tree("python3-pdm", None),
            tree(
                "python3-pdm",
                Some("[build-system]\n# build-backend = \"pdm.pep517.api\"\n"),
            ),
        ];
        let fail = vec![
            tree("python3-pdm", Some(PDM)),
            tree("python3-pdm (>= 2), debhelper-compat (= 14)", Some(PDM)),
            tree(
                "python3-pdm",
                Some("[build-system]\nbuild-backend = 'pdm.pep517.api'\n"),
            ),
        ];
        SourceTreeTester::new(UsesPdmCli, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree(
            "python3-pdm-backend",
            Some(PDM),
        ))];
        let fail = vec![SourcePackageCase::from(tree("python3-pdm", Some(PDM)))];
        SourcePackageTester::new(UsesPdmCli, pass, fail).test_and_snapshot();
    }
}
