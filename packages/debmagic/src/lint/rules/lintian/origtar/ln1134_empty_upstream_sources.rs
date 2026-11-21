use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;
use crate::lint::rules::lintian::fields_version::is_native;
use crate::lint::subject::SubjectKind;

pub struct EmptyUpstreamSources;

declare_rule! {
    /// A non-native Source has no upstream tarball entries.
    ///
    /// Native packages are skipped (lintian's `native`, see
    /// `fields/version`). A Source package is reported when its Orig files
    /// have no entries. A Source tree has no upstream tarball, so its Orig
    /// files count as empty and a non-native tree is reported too, as
    /// lintian's tree mode does. Extra is empty; the Subject is the location,
    /// so there is no Location.
    EmptyUpstreamSources,
    code = "LN1134",
    tag = "empty-upstream-sources",
    default_selected = true,
    experimental = false,
}

impl SourceRule for EmptyUpstreamSources {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        if is_native(ctx) {
            return;
        }
        let empty = match ctx.kind() {
            SubjectKind::SourceTree => true,
            _ => ctx
                .orig_files()
                .is_some_and(|orig| orig.entries().is_empty()),
        };
        if empty {
            ctx.diagnostic(Diagnostic::error(""));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestCase};

    use super::*;

    fn dsc(format: &str, version: &str) -> Dsc {
        Dsc::parse(&format!(
            "Format: {format}\nSource: example\nVersion: {version}\n"
        ))
        .expect("Dsc")
    }

    #[test]
    fn tree() {
        let pass = vec![
            TestCase::source_tree(vec![("debian/source/format", "3.0 (native)")]),
            TestCase::source_tree(vec![
                ("debian/source/format", "3.0 (native)"),
                ("debian/changelog", "example (1.0) unstable; urgency=low\n"),
            ]),
        ];
        let fail = vec![
            TestCase::source_tree(vec![("debian/source/format", "3.0 (quilt)")]),
            TestCase::source_tree(vec![
                ("debian/source/format", "3.0 (quilt)"),
                ("upstream", "p"),
            ]),
        ];
        SourceTreeTester::new(EmptyUpstreamSources, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source() {
        let pass = vec![
            // Native: no Orig files are expected.
            SourcePackageCase::from(vec![("upstream", "p")]).dsc(dsc("3.0 (native)", "1.0")),
            SourcePackageCase::from(vec![("debian/source/format", "3.0 (native)")])
                .dsc(dsc("3.0 (native)", "1.0")),
            // Non-native with an upstream tarball that has entries.
            SourcePackageCase::from(vec![("upstream", "p")])
                .orig(vec![("upstream", "o")])
                .dsc(dsc("3.0 (quilt)", "1.0-1")),
        ];
        let fail = vec![
            SourcePackageCase::from(vec![("upstream", "p")]).dsc(dsc("3.0 (quilt)", "1.0-1")),
            SourcePackageCase::from(vec![("upstream", "p")]).dsc(dsc("2.0", "1.0-1")),
            SourcePackageCase::from(vec![("upstream", "p")]).dsc(dsc("3.0 (git)", "1.0-1")),
        ];
        SourcePackageTester::new(EmptyUpstreamSources, pass, fail).test_and_snapshot();
    }
}
