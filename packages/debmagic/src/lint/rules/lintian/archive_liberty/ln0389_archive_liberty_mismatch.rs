use std::path::Path;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct ArchiveLibertyMismatch;

declare_rule! {
    /// A source and its installation packages are not in the same archive area.
    ///
    /// The area is the component before `/` in `Section`, and `main` when the
    /// field has no component. A source in `main` may ship `contrib` packages,
    /// and a source in `non-free` may ship `non-free-firmware`. When every
    /// installation package is in one inferior area and none stay with the
    /// source, the source paragraph is reported as well. Extra is
    /// `(in section for <package>) <area> vs <source area>` or
    /// `(in source paragraph) <source area> -> <area>`. The Location is
    /// `debian/control`.
    ArchiveLibertyMismatch,
    code = "LN0389",
    tag = "archive-liberty-mismatch",
    default_selected = true,
    experimental = false,
}

const INFERIOR: &[&str] = &["non-free", "contrib"];

impl SourceRule for ArchiveLibertyMismatch {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let Some(source_section) = control.source().get("Section") else {
            return;
        };
        if source_section.is_empty() {
            return;
        }
        let source_liberty = liberty(source_section);
        let mut areas = Vec::new();
        let mut hits = Vec::new();
        for installable in control.installables() {
            let fields = installable.fields();
            let section = fields
                .get("Section")
                .filter(|section| !section.is_empty())
                .unwrap_or(source_section);
            let area = liberty(section);
            let skipped = (source_liberty == "main" && area == "contrib")
                || (source_liberty == "non-free" && area == "non-free-firmware");
            if !skipped && area != source_liberty {
                hits.push(format!(
                    "(in section for {}) {area} vs {source_liberty}",
                    installable.name()
                ));
            }
            areas.push(area);
        }
        for inferior in INFERIOR {
            if *inferior == source_liberty {
                break;
            }
            if !areas.is_empty()
                && areas.iter().all(|area| area == inferior)
                && areas.iter().all(|area| area != &source_liberty)
            {
                hits.push(format!(
                    "(in source paragraph) {source_liberty} -> {inferior}"
                ));
            }
        }
        for extra in hits {
            ctx.diagnostic(Diagnostic::error(extra).with_location(Location {
                path: Path::new("debian/control").to_path_buf(),
                line: None,
                column: None,
            }));
        }
    }
}

/// Policy 2.4: a section without a component is in `main`. The area is the
/// component before the first `/`.
fn liberty(section: &str) -> String {
    let qualified = if section.contains('/') {
        section.to_string()
    } else {
        format!("main/{section}")
    };
    qualified
        .split_once('/')
        .map(|(area, _)| area.to_string())
        .unwrap_or(qualified)
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn control(body: &str) -> Vec<(String, TestFile)> {
        vec![("debian/control".to_string(), TestFile::new(body))]
    }

    #[test]
    fn test() {
        let pass = vec![
            control(
                "Source: example\nSection: libs\n\nPackage: example\nArchitecture: any\nDescription: example\n extra\n",
            ),
            control(
                "Source: example\nSection: contrib/libs\n\nPackage: example\nSection: contrib/net\nArchitecture: any\nDescription: example\n extra\n",
            ),
            control(
                "Source: example\nSection: libs\n\nPackage: example\nSection: contrib/libs\nArchitecture: any\nDescription: example\n extra\n\nPackage: example-main\nSection: libs\nArchitecture: any\nDescription: example\n extra\n",
            ),
            control(
                "Source: example\nSection: non-free/libs\n\nPackage: example\nSection: non-free-firmware/kernel\nArchitecture: any\nDescription: example\n extra\n",
            ),
        ];
        let fail = vec![
            control(
                "Source: example\nSection: contrib/libs\n\nPackage: example-main\nSection: libs\nArchitecture: any\nDescription: example\n extra\n",
            ),
            control(
                "Source: example\nSection: libs\n\nPackage: example-nonfree\nSection: non-free/libs\nArchitecture: any\nDescription: example\n extra\n",
            ),
            control(
                "Source: example\nSection: libs\n\nPackage: example-contrib\nSection: contrib/libs\nArchitecture: any\nDescription: example\n extra\n",
            ),
        ];
        SourceTreeTester::new(ArchiveLibertyMismatch, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(control(
            "Source: example\nSection: libs\n\nPackage: example\nArchitecture: any\nDescription: example\n extra\n",
        ))];
        let fail = vec![SourcePackageCase::from(control(
            "Source: example\nSection: contrib/libs\n\nPackage: example-main\nSection: libs\nArchitecture: any\nDescription: example\n extra\n",
        ))];
        SourcePackageTester::new(ArchiveLibertyMismatch, pass, fail).test_and_snapshot();
    }
}
