//! `debian/version-substvars` binNMU relation checks.

use std::sync::LazyLock;

use regex::Regex;

use crate::lint::debian_control::DebianControl;

static DEP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"([a-z0-9][-+.a-z0-9]+)(?::any)?\s*\(\s*([>]?=)\s*\$\{((?:Source-|source:|binary:)Version)\}",
    )
    .expect("valid regex")
});
static COMMA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*,\s*").expect("valid regex"));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Binnmu {
    AllDependsAny,
    AnyDependsAny,
    AnyDependsAll,
}

#[derive(Debug)]
pub(super) struct BinnmuHit {
    pub kind: Binnmu,
    pub message: String,
}

pub(super) fn hits(control: &DebianControl) -> Vec<BinnmuHit> {
    let mut found = Vec::new();
    for installable in control.installables() {
        let fields = installable.fields();
        let mut relations = Vec::new();
        for field in ["Pre-Depends", "Depends"] {
            if let Some(value) = fields.unfolded_value(field) {
                for part in COMMA.split(&value) {
                    let part = part.trim();
                    if !part.is_empty() && !relations.iter().any(|seen: &String| seen == part) {
                        relations.push(part.to_string());
                    }
                }
            }
        }
        let installable_is_all = fields.get("Architecture").is_some_and(|arch| arch == "all");
        for relation in relations {
            let Some(captures) = DEP.captures(&relation) else {
                continue;
            };
            let prerequisite = captures[1].to_string();
            let operator = captures[2].to_string();
            let substvar = captures[3].to_string();
            let Some(other) = control
                .installables()
                .iter()
                .find(|item| item.name() == prerequisite)
            else {
                continue;
            };
            if !other.fields().declares("Architecture") {
                continue;
            }
            let prerequisite_is_all = other
                .fields()
                .get("Architecture")
                .is_some_and(|arch| arch == "all");
            let message = format!("{} -> {prerequisite}", installable.name());
            if !installable_is_all
                && !prerequisite_is_all
                && operator == "="
                && substvar == "source:Version"
            {
                found.push(BinnmuHit {
                    kind: Binnmu::AnyDependsAny,
                    message: message.clone(),
                });
            }
            if !installable_is_all && prerequisite_is_all && substvar != "source:Version" {
                found.push(BinnmuHit {
                    kind: Binnmu::AnyDependsAll,
                    message: message.clone(),
                });
            }
            if installable_is_all && !prerequisite_is_all && operator == "=" {
                found.push(BinnmuHit {
                    kind: Binnmu::AllDependsAny,
                    message,
                });
            }
        }
    }
    found
}
