use crate::declare_rule;
use crate::lint::context::{SourceContext, follow_symlinks};
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

use super::ln1403_missing_prerequisite_for_pyproject_backend::build_backend;

pub struct UsesPoetryCli;

declare_rule! {
    /// `pyproject.toml` names `poetry.core.masonry.api` as its `build-backend` and
    /// `Build-Depends` satisfies `python3-poetry` (the command-line tool) but not
    /// `python3-poetry-core`, the prerequisite the backend needs. Reported at
    /// info severity, at the `build-backend` line, with an empty extra.
    ///
    /// `Build-Depends` alone is read, with its architecture, version and
    /// profile restrictions dropped, as lintian's `load_norestriction`. The
    /// `pyproject.toml` is followed through symlinks inside the tree and is
    /// skipped when it is not an open file. The sibling for pdm is
    /// LN1685 (`uses-pdm-cli`).
    UsesPoetryCli,
    code = "LN1686",
    tag = "uses-poetry-cli",
    default_selected = false,
    experimental = false,
}

impl SourceRule for UsesPoetryCli {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let build_depends = control
            .source()
            .relation("Build-Depends")
            .map(|relation| relation.restriction_less())
            .unwrap_or_default();
        if !build_depends.satisfies("python3-poetry")
            || build_depends.satisfies("python3-poetry-core")
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
                build_backend(&String::from_utf8_lossy(line)) == Some("poetry.core.masonry.api")
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

    const POETRY: &str = "[build-system]\n\
                       requires = [\"poetry-core\"]\n\
                       build-backend = \"poetry.core.masonry.api\"\n";

    const SETUPTOOLS: &str = "[build-system]\n\
                              build-backend = \"setuptools.build_meta\"\n";

    #[test]
    fn test() {
        let pass = vec![
            tree("debhelper-compat (= 14), python3-poetry-core", Some(POETRY)),
            tree(
                "debhelper-compat (= 14), python3-poetry, python3-poetry-core",
                Some(POETRY),
            ),
            tree("python3-poetry", Some(SETUPTOOLS)),
            tree("python3-poetry", None),
            tree(
                "python3-poetry",
                Some("[build-system]\n# build-backend = \"poetry.core.masonry.api\"\n"),
            ),
        ];
        let fail = vec![
            tree("python3-poetry", Some(POETRY)),
            tree(
                "python3-poetry (>= 2), debhelper-compat (= 14)",
                Some(POETRY),
            ),
            tree(
                "python3-poetry",
                Some("[build-system]\nbuild-backend = 'poetry.core.masonry.api'\n"),
            ),
        ];
        SourceTreeTester::new(UsesPoetryCli, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree(
            "python3-poetry-core",
            Some(POETRY),
        ))];
        let fail = vec![SourcePackageCase::from(tree(
            "python3-poetry",
            Some(POETRY),
        ))];
        SourcePackageTester::new(UsesPoetryCli, pass, fail).test_and_snapshot();
    }
}
