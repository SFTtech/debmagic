//! Name overlap for `conflicts-with-dependency` and
//! `build-conflicts-with-build-dependency`.
//!
//! This is the part of lintian's `satisfies` that the alternative list can
//! express: the same package name, architecture restrictions, multi-arch
//! qualifiers, and whether a version constraint is present. Two version
//! constraints are not ordered, so a versioned conflict is not treated as
//! implied. Build profiles are not implied either, matching lintian's refusal
//! there.

use std::sync::LazyLock;

use regex::Regex;

use crate::lint::debian_control::ControlParagraph;
use crate::lint::package_relation::{Alternative, PackageRelation};

static COMMA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*,\s*").expect("valid regex"));

/// Comma-separated relation items, trailing empties dropped, each end trimmed.
pub(super) fn comma_separated(value: &str) -> Vec<&str> {
    let mut items: Vec<&str> = COMMA.split(value.trim()).collect();
    while items.last() == Some(&"") {
        items.pop();
    }
    items
        .into_iter()
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .collect()
}

/// `Build-Depends`, `Build-Depends-Indep`, and `Build-Depends-Arch` as one
/// AND, lintian's `Build-Depends-All`.
pub(super) fn build_depends_all(fields: &ControlParagraph) -> PackageRelation {
    let text = ["Build-Depends", "Build-Depends-Indep", "Build-Depends-Arch"]
        .into_iter()
        .filter_map(|field| fields.unfolded_value(field))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(", ");
    PackageRelation::parse(&text)
}

/// Whether every alternative of `item` is implied by `depend`.
///
/// `item` is one comma-separated conflict as written. A version constraint on
/// that conflict is never implied: ordering two versions needs a comparison
/// this crate does not have. An unversioned conflict is implied when some
/// dependency clause names the same package and its architecture restriction
/// and `:arch` qualifier are at least as general.
pub(super) fn dependency_implies(depend: &PackageRelation, item: &str) -> bool {
    let target = PackageRelation::parse(item);
    let required = target.dependencies();
    if required.is_empty() {
        return false;
    }
    required.iter().all(|clause| {
        !clause.is_empty()
            && depend
                .dependencies()
                .iter()
                .any(|have| clause_implies(have, clause))
    })
}

fn clause_implies(depend: &[Alternative], conflict: &[Alternative]) -> bool {
    if depend.is_empty() || conflict.is_empty() {
        return false;
    }
    if conflict.len() == 1 {
        let conflict = &conflict[0];
        if depend.len() == 1 {
            predicate_implies(&depend[0], conflict)
        } else {
            depend.iter().all(|item| predicate_implies(item, conflict))
        }
    } else if depend.len() == 1 {
        conflict
            .iter()
            .any(|item| predicate_implies(&depend[0], item))
    } else {
        depend
            .iter()
            .all(|left| conflict.iter().any(|right| predicate_implies(left, right)))
    }
}

fn predicate_implies(depend: &Alternative, conflict: &Alternative) -> bool {
    if depend.package().is_empty() || depend.package() != conflict.package() {
        return false;
    }
    if !depend.profiles().is_empty() || !conflict.profiles().is_empty() {
        return false;
    }
    if !architectures_allow(depend, conflict) || !multiarch_allows(depend, conflict) {
        return false;
    }
    conflict.version().is_none()
}

/// `depend`'s architecture restriction is a superset of `conflict`'s.
fn architectures_allow(depend: &Alternative, conflict: &Alternative) -> bool {
    let left = depend.architectures();
    let right = conflict.architectures();
    if left.is_empty() {
        return true;
    }
    if right.is_empty() {
        return false;
    }
    let left_neg = depend.negated_architectures() == left.len();
    let right_neg = conflict.negated_architectures() == right.len();
    if (depend.negated_architectures() != 0 && !left_neg)
        || (conflict.negated_architectures() != 0 && !right_neg)
    {
        return false;
    }
    if !left_neg && !right_neg {
        return right.iter().all(|arch| left.contains(arch));
    }
    if left_neg && right_neg {
        return left.iter().all(|arch| right.contains(arch));
    }
    if !left_neg && right_neg {
        return false;
    }
    left.iter().all(|excluded| !right.contains(excluded))
}

fn multiarch_allows(depend: &Alternative, conflict: &Alternative) -> bool {
    let left = depend.architecture_qualifier();
    let right = conflict.architecture_qualifier();
    if left.is_empty() && !right.is_empty() && right != "any" {
        return false;
    }
    if !left.is_empty() && right.is_empty() {
        return false;
    }
    if !left.is_empty() && !right.is_empty() && left != right {
        return false;
    }
    true
}

/// First physical line of `field` in the source paragraph of `debian/control`.
pub(super) fn source_field_line(text: &str, field: &str) -> Option<u32> {
    for (index, raw) in text.split('\n').enumerate() {
        let line = raw.trim_end_matches('\r');
        if line.trim().is_empty() {
            break;
        }
        if line.starts_with([' ', '\t']) || line.trim_start().starts_with('#') {
            continue;
        }
        let Some((name, _)) = line.split_once(':') else {
            continue;
        };
        if name.eq_ignore_ascii_case(field) {
            return u32::try_from(index + 1).ok();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recipe_relations_are_name_and_architecture() {
        let build = PackageRelation::parse(
            "foo (= 3) [!amd64 !i386], bar, debmake, arch-test1 [i386], arch-test2 [!i386]",
        );
        assert!(!dependency_implies(&build, "foo [amd64 i386]"));
        assert!(dependency_implies(&build, "bar [alpha test]"));
        assert!(dependency_implies(&build, "debmake [!powerpc]"));
        assert!(!dependency_implies(&build, "arch-test1 [powerpc]"));
        assert!(!dependency_implies(&build, "arch-test2 [!sparc]"));
        assert!(!dependency_implies(&build, "xlibs-dev"));

        let binary = PackageRelation::parse(
            "relations (<< 3), foo (>> 2.0), foo (<< 2.2), libpng2, libpng3",
        );
        assert!(dependency_implies(&binary, "relations"));
        assert!(dependency_implies(&binary, "foo"));
        assert!(dependency_implies(&binary, "libpng2"));
        assert!(!dependency_implies(&binary, "libpng3 (<< 1.0)"));
        assert!(!dependency_implies(&binary, "foobar (<< 5+5)"));
    }

    #[test]
    fn or_and_profiles_and_qualifiers() {
        let either = PackageRelation::parse("bar | foo");
        assert!(!dependency_implies(&either, "foo"));
        let foo = PackageRelation::parse("foo");
        assert!(dependency_implies(&foo, "foo | bar"));
        assert!(!dependency_implies(&foo, "foo (>= 1)"));
        let profile = PackageRelation::parse("foo <nocheck>");
        assert!(!dependency_implies(&profile, "foo"));
        let qualified = PackageRelation::parse("foo:any");
        assert!(!dependency_implies(&qualified, "foo"));
        assert!(dependency_implies(&foo, "foo:any"));
    }
}
