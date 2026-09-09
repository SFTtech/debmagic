use std::collections::HashSet;

use anyhow::{Context, bail};

use super::registry;
use super::rule::Code;

#[derive(Debug, Clone, PartialEq, Eq)]
enum CodeSelector {
    Exact(Code),
    Prefix(String),
}

fn parse_code_selector(selector: &str) -> anyhow::Result<Option<CodeSelector>> {
    let bytes = selector.as_bytes();
    if bytes.len() < 2 || !bytes[0].is_ascii_alphabetic() || !bytes[1].is_ascii_alphabetic() {
        return Ok(None);
    }

    if !bytes[2..].iter().all(u8::is_ascii_digit) {
        return Ok(None);
    }
    if bytes.len() > 6 {
        bail!("invalid code selector '{selector}'");
    }

    if bytes.len() == 6 {
        Ok(Some(CodeSelector::Exact(
            Code::parse(selector).with_context(|| format!("invalid code selector '{selector}'"))?,
        )))
    } else {
        Ok(Some(CodeSelector::Prefix(selector.to_ascii_uppercase())))
    }
}

fn codes_for_selector(selector: &str) -> anyhow::Result<Vec<Code>> {
    if let Some(code_selector) = parse_code_selector(selector)? {
        return Ok(match code_selector {
            CodeSelector::Exact(code) => {
                let matches = registry::all_rules()
                    .iter()
                    .copied()
                    .filter(|rule| rule.code() == code)
                    .map(|rule| rule.code())
                    .collect::<Vec<_>>();
                if matches.is_empty() {
                    bail!("unknown selector '{selector}'");
                }
                matches
            }
            CodeSelector::Prefix(prefix) => registry::all_rules()
                .iter()
                .copied()
                .filter(|rule| rule.code().matches_selector_prefix(&prefix))
                .map(|rule| rule.code())
                .collect(),
        });
    }

    let matches = registry::all_rules()
        .iter()
        .copied()
        .filter(|rule| rule.tag().as_str() == selector)
        .map(|rule| rule.code())
        .collect::<Vec<_>>();
    if matches.is_empty() {
        bail!("unknown selector '{selector}'");
    }
    Ok(matches)
}

pub(crate) fn default_selected_codes() -> Vec<Code> {
    registry::all_rules()
        .iter()
        .copied()
        .filter(|rule| !rule.experimental() && rule.default_selected())
        .map(|rule| rule.code())
        .collect()
}

pub(crate) fn resolve_selected_codes(
    select: &[String],
    ignore: &[String],
) -> anyhow::Result<Vec<Code>> {
    let mut selected = if select.is_empty() {
        default_selected_codes()
    } else {
        let mut codes = HashSet::new();
        for selector in select {
            for code in codes_for_selector(selector)? {
                codes.insert(code);
            }
        }
        let mut codes = codes.into_iter().collect::<Vec<_>>();
        codes.sort();
        codes
    };

    if !ignore.is_empty() {
        let mut ignored = HashSet::new();
        for selector in ignore {
            for code in codes_for_selector(selector)? {
                ignored.insert(code);
            }
        }
        selected.retain(|code| !ignored.contains(code));
    }

    Ok(selected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lint::rule::RuleMeta;
    use crate::lint::rules::{DebmagicDummyTrigger, RequiredField};

    #[test]
    fn default_selection_includes_dm0001_and_ln0001() -> anyhow::Result<()> {
        let selected = resolve_selected_codes(&[], &[])?;
        assert!(selected.contains(&DebmagicDummyTrigger::CODE));
        assert!(selected.contains(&RequiredField::CODE));
        Ok(())
    }

    #[test]
    fn select_exact_code() -> anyhow::Result<()> {
        let selected = resolve_selected_codes(&["DM0001".to_string()], &[])?;
        assert_eq!(selected, vec![DebmagicDummyTrigger::CODE]);
        Ok(())
    }

    #[test]
    fn select_unknown_prefix_is_empty() -> anyhow::Result<()> {
        let selected = resolve_selected_codes(&["ZZ".to_string()], &[])?;
        assert!(selected.is_empty());
        Ok(())
    }

    #[test]
    fn select_ln_prefix_selects_ln_rules() -> anyhow::Result<()> {
        use crate::lint::rules::SyntaxErrorInDebianChangelog;
        let selected = resolve_selected_codes(&["LN".to_string()], &[])?;
        assert!(selected.contains(&RequiredField::CODE));
        assert!(selected.contains(&SyntaxErrorInDebianChangelog::CODE));
        assert!(selected.contains(&crate::lint::rules::DebianRulesMissingRequiredTarget::CODE));
        assert!(selected.iter().all(|code| code.prefix() == "LN"));
        Ok(())
    }

    #[test]
    fn ignore_code_removes_from_default() -> anyhow::Result<()> {
        let selected = resolve_selected_codes(&[], &["DM0001".to_string()])?;
        assert!(!selected.contains(&DebmagicDummyTrigger::CODE));
        Ok(())
    }

    #[test]
    fn ignore_tag_removes_from_default() -> anyhow::Result<()> {
        let selected = resolve_selected_codes(&[], &["debmagic-dummy-trigger".to_string()])?;
        assert!(!selected.contains(&DebmagicDummyTrigger::CODE));
        Ok(())
    }

    #[test]
    fn select_dm_prefix_selects_dummy() -> anyhow::Result<()> {
        let selected = resolve_selected_codes(&["DM".to_string()], &[])?;
        assert_eq!(selected, vec![DebmagicDummyTrigger::CODE]);
        Ok(())
    }

    #[test]
    fn unknown_selector_errors() {
        let error = resolve_selected_codes(&["not-a-real-selector".to_string()], &[]).unwrap_err();
        assert!(error.to_string().contains("unknown selector"));
    }

    #[test]
    fn unknown_exact_code_errors() {
        let error = resolve_selected_codes(&["DM9999".to_string()], &[]).unwrap_err();
        assert!(error.to_string().contains("unknown selector"));
    }
}
