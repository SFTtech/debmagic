use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::SOURCE_RELATION_FIELDS;

pub struct BuildDependsOnBuildEssentialPackageWithoutUsingVersion;

declare_rule! {
    /// A source relation names a build-essential package and gives no version.
    ///
    /// `Build-Depends`, `Build-Depends-Indep`, `Build-Depends-Arch`,
    /// `Build-Conflicts`, `Build-Conflicts-Indep`, and `Build-Conflicts-Arch`
    /// of the source paragraph. The package list is lintian's
    /// `fields/build-essential-packages`. `PackageRelation::version()` being
    /// absent is the whole check; versions are not compared. Extra is the
    /// package, then the field and the alternative as written.
    BuildDependsOnBuildEssentialPackageWithoutUsingVersion,
    code = "LN0352",
    tag = "build-depends-on-build-essential-package-without-using-version",
    default_selected = true,
    experimental = false,
}

/// `data/fields/build-essential-packages` (lintian 2.139.0).
const BUILD_ESSENTIAL: &[&str] = &["libc6-dev", "libc-dev", "gcc", "g++", "make", "dpkg-dev"];

impl SourceRule for BuildDependsOnBuildEssentialPackageWithoutUsingVersion {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let messages = ctx
            .debian_control()
            .map(|control| messages(control.source()))
            .unwrap_or_default();
        for message in messages {
            ctx.diagnostic(Diagnostic::error(message));
        }
    }
}

fn messages(fields: &ControlParagraph) -> Vec<String> {
    SOURCE_RELATION_FIELDS
        .iter()
        .filter_map(|field| Some((*field, fields.relation(field)?)))
        .flat_map(|(field, relation)| {
            relation
                .alternatives()
                .filter(|alternative| {
                    alternative.version().is_none()
                        && BUILD_ESSENTIAL.contains(&alternative.package())
                })
                .map(|alternative| {
                    format!(
                        "{} [{field}: {}]",
                        alternative.package(),
                        alternative.text()
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn control(fields: &[(&str, &str)]) -> Vec<(&'static str, TestFile)> {
        let mut body = String::from(
            "Source: example\nMaintainer: Example <ex@example.com>\n\
             Standards-Version: 4.7.2\nSection: devel\n",
        );
        for (name, value) in fields {
            body.push_str(name);
            body.push_str(": ");
            body.push_str(value);
            body.push('\n');
        }
        body.push_str("\nPackage: example\nArchitecture: all\nDescription: example\n extra\n");
        vec![("debian/control", TestFile::new(body))]
    }

    #[test]
    fn test() {
        let installable_only = vec![(
            "debian/control",
            TestFile::new(
                "Source: example\n\n\
                 Package: example\nArchitecture: all\nDepends: make\n\
                 Description: example\n extra\n",
            ),
        )];
        let pass = vec![
            Vec::<(&str, TestFile)>::new(),
            control(&[]),
            control(&[
                (
                    "Build-Depends",
                    "debhelper, make (>= 4), gcc (>= 4), g++ (>= 12)",
                ),
                ("Build-Depends-Indep", "libc6-dev (>= 2)"),
            ]),
            control(&[("Build-Depends", "build-essential, makedev, libc6")]),
            installable_only,
        ];
        let fail = vec![
            control(&[
                (
                    "Build-Depends",
                    "make, libc6-dev (>= 1), gcc [amd64], foo | g++",
                ),
                ("Build-Depends-Indep", "dpkg-dev"),
                ("Build-Depends-Arch", "libc-dev:native"),
                ("Build-Conflicts", "make"),
            ]),
            control(&[("Build-Depends", "make (4.0)")]),
        ];
        SourceTreeTester::new(
            BuildDependsOnBuildEssentialPackageWithoutUsingVersion,
            pass,
            fail,
        )
        .test_and_snapshot();
    }
}
