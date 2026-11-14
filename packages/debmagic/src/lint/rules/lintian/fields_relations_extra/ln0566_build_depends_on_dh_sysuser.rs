use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::{SOURCE_DEPENDENCY_FIELDS, alternatives_of};

pub struct BuildDependsOnDhSysuser;

declare_rule! {
    /// A build dependency names `dh-sysuser` outside the `runit` source.
    ///
    /// `Build-Depends`, `Build-Depends-Indep`, and `Build-Depends-Arch`. The
    /// canonical replacement is `dh-sequence-installsysusers`. Skipped when
    /// the `Source` field is `runit`. Extra is the field and the alternative
    /// as written.
    BuildDependsOnDhSysuser,
    code = "LN0566",
    tag = "build-depends-on-dh-sysuser",
    default_selected = true,
    experimental = false,
}

impl SourceRule for BuildDependsOnDhSysuser {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let messages: Vec<String> = ctx
            .debian_control()
            .filter(|control| control.source().get("Source") != Some("runit"))
            .map(|control| {
                alternatives_of(control.source(), SOURCE_DEPENDENCY_FIELDS)
                    .into_iter()
                    .filter(|(_, alternative)| alternative.package() == "dh-sysuser")
                    .map(|(field, alternative)| format!("{field}: {}", alternative.text()))
                    .collect()
            })
            .unwrap_or_default();
        for message in messages {
            ctx.diagnostic(Diagnostic::error(message));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::super::testing::{control, raw};
    use super::*;

    #[test]
    fn test() {
        let pass = vec![
            Vec::<(&str, TestFile)>::new(),
            control(&[("Build-Depends", "debhelper, dh-sequence-installsysusers")]),
            control(&[("Build-Conflicts", "dh-sysuser")]),
            raw("Source: runit\nBuild-Depends: dh-sysuser\n"),
        ];
        let fail = vec![
            control(&[("Build-Depends", "debhelper, dh-sysuser (>= 1)")]),
            control(&[
                ("Build-Depends-Indep", "foo | dh-sysuser"),
                ("Build-Depends-Arch", "dh-sysuser:any"),
            ]),
        ];
        SourceTreeTester::new(BuildDependsOnDhSysuser, pass, fail).test_and_snapshot();
    }
}
