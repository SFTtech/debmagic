use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::obsolete_bootstrap_messages;

pub struct BuildDependsOnObsoleteBootstrap;

declare_rule! {
    /// A build-dependency names an obsolete Bootstrap package.
    ///
    /// `Build-Depends`, `Build-Depends-Indep`, and `Build-Depends-Arch`. The
    /// names are `libjs-bootstrap`, `fonts-glyphicons-halflings`, and
    /// `libjs-bootstrap4`, in any alternative. Build-conflicts are not this
    /// Tag. A version constraint does not matter. Extra is the field and the
    /// package name. Applies to the Source tree and the Source package.
    BuildDependsOnObsoleteBootstrap,
    code = "LN0268",
    tag = "build-depends-on-obsolete-bootstrap",
    default_selected = true,
    experimental = false,
}

const FIELDS: &[&str] = &["Build-Depends", "Build-Depends-Indep", "Build-Depends-Arch"];

impl SourceRule for BuildDependsOnObsoleteBootstrap {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let messages = ctx
            .debian_control()
            .map(|control| obsolete_bootstrap_messages(control.source(), FIELDS))
            .unwrap_or_default();
        for message in messages {
            ctx.diagnostic(Diagnostic::warn(message));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn control(fields: &[(&str, &str)]) -> Vec<(&'static str, TestFile)> {
        let mut body = String::from("Source: example\n");
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
                 Package: example\nArchitecture: all\nDepends: libjs-bootstrap\n\
                 Description: example\n extra\n",
            ),
        )];
        let pass = vec![
            Vec::<(&str, TestFile)>::new(),
            control(&[("Build-Depends", "debhelper, libjs-bootstrap5")]),
            control(&[("Build-Conflicts", "libjs-bootstrap")]),
            installable_only,
        ];
        let fail = vec![control(&[
            (
                "Build-Depends",
                "libjs-bootstrap, foo | fonts-glyphicons-halflings (>= 1)",
            ),
            ("Build-Depends-Indep", "libjs-bootstrap4"),
            ("Build-Depends-Arch", "libjs-bootstrap:any"),
        ])];
        SourceTreeTester::new(BuildDependsOnObsoleteBootstrap, pass, fail).test_and_snapshot();
    }
}
