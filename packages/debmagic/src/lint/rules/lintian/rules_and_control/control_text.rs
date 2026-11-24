//! Build-dependency names of `debian/control`.
//!
//! Name presence walks `Build-Depends`, `Build-Depends-Indep`, and
//! `Build-Depends-Arch` (lintian's `Build-Depends-All`) and compares package
//! names, ignoring versions, architecture qualifiers, and restrictions.

use crate::lint::debian_control::ControlParagraph;

const BUILD_DEPENDS_ALL: &[&str] = &["Build-Depends", "Build-Depends-Indep", "Build-Depends-Arch"];

/// Package names declared by the source paragraph's build-dependency fields.
pub(in super::super) fn build_depends_all_names(source: &ControlParagraph) -> Vec<String> {
    let mut names = Vec::new();
    for field in BUILD_DEPENDS_ALL {
        let Some(relation) = source.relation(field) else {
            continue;
        };
        for alternative in relation.alternatives() {
            if !alternative.package().is_empty() {
                names.push(alternative.package().to_string());
            }
        }
    }
    names
}

pub(in super::super) fn names_package(names: &[String], package: &str) -> bool {
    names.iter().any(|name| name == package)
}

#[cfg(test)]
mod tests {
    use crate::lint::debian_control::DebianControl;

    use super::*;

    #[test]
    fn build_depends_all_names_and_field_lines_follow_the_file() {
        let text = "\
Source: example\n\
Build-Depends: debhelper\n\
\n\
# comment\n\
Package: foo++\n\
Architecture: all\n\
Build-Depends: test\n\
Description: see\n extra\n";
        let control = DebianControl::parse(text).expect("control");
        let installable = &control.installables()[0];
        assert_eq!(installable.name(), "foo++");
        assert_eq!(installable.fields().position("Package"), Some(5));
        assert_eq!(installable.fields().position("Build-Depends"), Some(7));
        assert!(names_package(
            &build_depends_all_names(control.source()),
            "debhelper"
        ));
    }
}
