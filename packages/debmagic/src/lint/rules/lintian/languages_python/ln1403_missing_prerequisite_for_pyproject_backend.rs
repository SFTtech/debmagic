use crate::declare_rule;
use crate::lint::context::{SourceContext, follow_symlinks};
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

pub struct MissingPrerequisiteForPyprojectBackend;

declare_rule! {
    /// `pyproject.toml` names a `build-backend` whose prerequisite is not in
    /// `Build-Depends`, `Build-Depends-Indep` or `Build-Depends-Arch`, or is
    /// satisfied only by the generic `pybuild-plugin-pyproject` missing.
    ///
    /// Only the backends in lintian's table are checked. The prerequisites are
    /// the table's package, then `pybuild-plugin-pyproject`, and the relation
    /// must be satisfied by all of the source build dependencies. Reported at
    /// the `build-backend` line, with extra `<backend> (does not satisfy
    /// <prerequisites>)`. A `pyproject.toml` that is not an open file (a
    /// symlink out of the tree) is skipped. Applies to the Source tree and the
    /// Source package, which reads the Patched files.
    MissingPrerequisiteForPyprojectBackend,
    code = "LN1403",
    tag = "missing-prerequisite-for-pyproject-backend",
    default_selected = false,
    experimental = false,
}

/// lintian's `%PYPROJECT_PREREQUISITES`, from `Lintian::Check::Languages::Python`.
const PREREQUISITES: &[(&str, &str)] = &[
    ("poetry.core.masonry.api", "python3-poetry-core"),
    ("flit_core.buildapi", "flit | python3-flit"),
    ("setuptools.build_meta", "python3-setuptools"),
    ("pdm.pep517.api", "python3-pdm-backend"),
    ("hatchling.build", "python3-hatchling"),
    ("mesonpy", "python3-mesonpy"),
    ("sipbuild.api", "python3-sipbuild"),
    ("whey", "python3-whey"),
];

impl SourceRule for MissingPrerequisiteForPyprojectBackend {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let build_all = match ctx.debian_control() {
            Some(control) => control.source().relations_and(&[
                "Build-Depends",
                "Build-Depends-Indep",
                "Build-Depends-Arch",
            ]),
            None => return,
        };
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

        let mut findings = Vec::new();
        for (index, line) in bytes.split(|byte| *byte == b'\n').enumerate() {
            let line = String::from_utf8_lossy(line);
            let Some(backend) = build_backend(&line) else {
                continue;
            };
            let Some((_, package)) = PREREQUISITES.iter().find(|(name, _)| *name == backend) else {
                continue;
            };
            let prerequisites = format!("{package}, pybuild-plugin-pyproject");
            if !build_all.satisfies(&prerequisites) {
                findings.push((
                    index as u32 + 1,
                    format!("{backend} (does not satisfy {prerequisites})"),
                ));
            }
        }
        for (line, message) in findings {
            ctx.diagnostic(
                Diagnostic::warn(message)
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

/// The backend named by a `build-backend = "..."` or `'...'` line. The
/// single-quoted form may not hold a double quote, as the Check's second
/// pattern requires.
fn build_backend(line: &str) -> Option<&str> {
    let rest = line.trim_start().strip_prefix("build-backend")?;
    let rest = rest.trim_start().strip_prefix('=')?.trim_start();
    let (quote, rest) = match rest.strip_prefix('"') {
        Some(rest) => ('"', rest),
        None => ('\'', rest.strip_prefix('\'')?),
    };
    let backend = &rest[..rest.find(quote)?];
    let valid = !backend.is_empty() && (quote == '"' || !backend.contains('"'));
    valid.then_some(backend)
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn tree(build_depends: &str, pyproject: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            (
                "debian/control",
                TestFile::new(format!(
                    "Source: example\nBuild-Depends: {build_depends}\n\nPackage: example\nArchitecture: all\nDescription: example\n extra\n"
                )),
            ),
            ("pyproject.toml", TestFile::new(pyproject)),
        ]
    }

    #[test]
    fn test() {
        let setuptools = "[build-system]\nbuild-backend = \"setuptools.build_meta\"\n";
        let pass = vec![
            // No pyproject.toml, no backend.
            vec![(
                "debian/control",
                TestFile::new(
                    "Source: example\n\nPackage: example\nArchitecture: all\nDescription: example\n extra\n",
                ),
            )],
            tree(
                "debhelper, python3-setuptools, pybuild-plugin-pyproject",
                setuptools,
            ),
            tree("python3-setuptools, pybuild-plugin-pyproject", setuptools),
            // A backend the table does not list.
            tree("debhelper", "build-backend = \"example.build\"\n"),
            // Single quotes, the flit prerequisite as an alternative.
            tree(
                "flit | python3-flit, pybuild-plugin-pyproject",
                "build-backend = 'flit_core.buildapi'\n",
            ),
        ];
        let fail = vec![
            tree("debhelper", setuptools),
            // The generic plugin is missing.
            tree("python3-setuptools", setuptools),
            // The prerequisite is missing on the second line only.
            tree(
                "python3-setuptools, pybuild-plugin-pyproject",
                "[build-system]\nrequires = []\nbuild-backend = \"hatchling.build\"\n",
            ),
            tree(
                "debhelper, pybuild-plugin-pyproject",
                "build-backend = 'flit_core.buildapi'\n",
            ),
        ];
        SourceTreeTester::new(MissingPrerequisiteForPyprojectBackend, pass, fail)
            .test_and_snapshot();
    }
}
