use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::control_text::{InstallableParagraph, installable_paragraphs};
use super::rules_text::read_utf8;

pub struct BuildPrerequisiteInInstallableSection;

declare_rule! {
    /// A source relationship field is declared on an installable.
    ///
    /// `Build-Depends`, `Build-Depends-Indep`, `Build-Conflicts`, and
    /// `Build-Conflicts-Indep` on a binary paragraph. `Build-Depends-Arch`
    /// and `Build-Conflicts-Arch` are not this Tag. Extra is the field name
    /// then `(in section for <package>)`. Location is `debian/control` at the
    /// field. Applies to the Source tree and Source package.
    BuildPrerequisiteInInstallableSection,
    code = "LN0223",
    tag = "build-prerequisite-in-installable-section",
    default_selected = true,
    experimental = false,
}

const BUILD_FIELDS: &[&str] = &[
    "Build-Depends",
    "Build-Depends-Indep",
    "Build-Conflicts",
    "Build-Conflicts-Indep",
];

impl SourceRule for BuildPrerequisiteInInstallableSection {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let installables: Vec<(String, Vec<&'static str>)> = ctx
            .debian_control()
            .map(|control| {
                control
                    .installables()
                    .iter()
                    .map(|installable| {
                        let fields = BUILD_FIELDS
                            .iter()
                            .copied()
                            .filter(|field| installable.fields().declares(field))
                            .collect();
                        (installable.name().to_string(), fields)
                    })
                    .filter(|(_, fields): &(_, Vec<_>)| !fields.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        if installables.is_empty() {
            return;
        }
        let paragraphs = ctx
            .files()
            .and_then(|files| read_utf8(files, "debian/control"))
            .map(|text| installable_paragraphs(&text))
            .unwrap_or_default();
        let mut cursor = 0;
        for (name, fields) in installables {
            let paragraph = take_paragraph(&paragraphs, &name, &mut cursor);
            for field in fields {
                let line = paragraph.and_then(|paragraph| paragraph.field_line(field));
                ctx.diagnostic(
                    Diagnostic::error(format!("{field} (in section for {name})")).with_location(
                        Location {
                            path: PathBuf::from("debian/control"),
                            line,
                            column: None,
                        },
                    ),
                );
            }
        }
    }
}

fn take_paragraph<'a>(
    paragraphs: &'a [InstallableParagraph],
    name: &str,
    cursor: &mut usize,
) -> Option<&'a InstallableParagraph> {
    let offset = paragraphs[*cursor..]
        .iter()
        .position(|paragraph| paragraph.package == name)?;
    let paragraph = &paragraphs[*cursor + offset];
    *cursor += offset + 1;
    Some(paragraph)
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn control(text: &str) -> Vec<(&'static str, TestFile)> {
        vec![("debian/control", TestFile::new(text))]
    }

    const PLAIN: &str = "\
Source: example\n\
Maintainer: Example <ex@example.com>\n\
Standards-Version: 4.7.2\n\
Build-Depends: debhelper\n\
\n\
Package: example\n\
Architecture: all\n\
Description: example\n extra\n";

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no control"))],
            control(PLAIN),
            control(
                "\
Source: example\n\
\n\
Package: example\n\
Architecture: all\n\
Build-Depends-Arch: debhelper\n\
Build-Conflicts-Arch: foo\n\
Description: example\n extra\n",
            ),
        ];
        let fail = vec![
            control(
                "\
Source: example\n\
Build-Depends: debhelper\n\
\n\
Package: example\n\
Architecture: all\n\
Build-Depends: again\n\
Description: example\n extra\n",
            ),
            control(
                "\
Source: foo++\n\
\n\
Package: foo++\n\
Architecture: all\n\
Build-Depends: test\n\
Description: plus\n signs\n",
            ),
            control(
                "\
Source: example\n\
\n\
Package: example\n\
Architecture: all\n\
Build-Depends-Indep: perl\n\
Build-Conflicts: foo\n\
Build-Conflicts-Indep: bar\n\
Description: example\n extra\n",
            ),
            // The pointer is the first line of a continued field.
            control(
                "Source: example\n\nPackage: example\nArchitecture: all\nBuild-Depends: foo,\n bar\nDescription: example\n extra\n",
            ),
            // The field name in the extra is the canonical spelling.
            control(
                "\
Source: example\n\
\n\
Package: left\n\
Architecture: all\n\
build-depends: foo\n\
Description: left\n package\n\
\n\
Package: right\n\
Architecture: all\n\
Build-Conflicts: bar\n\
Description: right\n package\n",
            ),
        ];
        SourceTreeTester::new(BuildPrerequisiteInInstallableSection, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(control(PLAIN))];
        let fail = vec![SourcePackageCase::from(control(
            "\
Source: example\n\
\n\
Package: example\n\
Architecture: all\n\
Build-Conflicts: foo\n\
Description: example\n extra\n",
        ))];
        SourcePackageTester::new(BuildPrerequisiteInInstallableSection, pass, fail)
            .test_and_snapshot();
    }
}
