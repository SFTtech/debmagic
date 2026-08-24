use super::rule::Rule;
use super::rules::{DebmagicDummyTrigger, RequiredField};
use super::subject::SubjectKind;

pub(crate) fn all_rules() -> &'static [&'static dyn Rule] {
    &[&DebmagicDummyTrigger, &RequiredField]
}

pub(crate) fn rule_applies(rule: &dyn Rule, kind: SubjectKind) -> bool {
    rule.applicability().contains(&kind)
}
