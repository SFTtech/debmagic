//! Shared check for `maintainer-script-needs-depends-on-*`.
//!
//! A strong dependency names the package when `Depends` or `Pre-Depends` has
//! that package name on any alternative, including with a version constraint,
//! `:arch` qualifier, architecture restriction, or build profile. The binary
//! package's own name counts too. `Recommends` and `Suggests` do not. Versions
//! are not compared.

use std::path::Path;

use regex::Regex;

use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};

use super::script::{Continuations, maintainer_script_lines};

/// `|| true` on the same line, with or without space around `||`.
static OR_TRUE: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"\|\|\s*true\b").expect("valid regex"));

/// One command that requires a package name in a strong dependency.
pub(super) struct CommandNeed {
    pattern: Regex,
    dash_x: Regex,
    which_or_type: Regex,
    command: Regex,
    package: &'static str,
}

impl CommandNeed {
    /// `pattern` is lintian's command regex, including the trailing `\s`.
    pub(super) fn new(pattern: &str, package: &'static str) -> Self {
        Self {
            pattern: Regex::new(pattern).expect("valid regex"),
            dash_x: Regex::new(&format!(r"-x\s+\S*{pattern}")).expect("valid regex"),
            which_or_type: Regex::new(&format!(r"(?:which|type)\s+{pattern}"))
                .expect("valid regex"),
            command: Regex::new(&format!(r"command\s+.*?{pattern}")).expect("valid regex"),
            package,
        }
    }

    pub(super) fn package(&self) -> &'static str {
        self.package
    }

    /// Command text when `line` calls it and no guard on that line exempts it.
    fn unguarded<'a>(&self, line: &'a str) -> Option<&'a str> {
        let found = self.pattern.find(line)?;
        if self.dash_x.is_match(line)
            || self.which_or_type.is_match(line)
            || self.command.is_match(line)
            || OR_TRUE.is_match(line)
        {
            return None;
        }
        let text = found.as_str().trim();
        if text.is_empty() { None } else { Some(text) }
    }
}

/// Whether `Depends`, `Pre-Depends`, or the binary package name is `package`.
pub(super) fn dependency_names(ctx: &BinaryPackageContext<'_>, package: &str) -> bool {
    if ctx.package_name() == package {
        return true;
    }
    let fields = ctx.installation_control().fields();
    ["Pre-Depends", "Depends"].into_iter().any(|field| {
        fields.relation(field).is_some_and(|relation| {
            relation
                .alternatives()
                .any(|alternative| alternative.package() == package)
        })
    })
}

/// Emit one warning per unguarded call when no strong dependency names the package.
///
/// `postrm` is exempt. Extra is the command and `(does not satisfy <package>)`.
pub(super) fn flag_missing_dependency(ctx: &mut BinaryPackageContext<'_>, need: &CommandNeed) {
    if dependency_names(ctx, need.package()) {
        return;
    }
    let Some(files) = ctx.control_files() else {
        return;
    };
    let hits: Vec<_> = maintainer_script_lines(files, Continuations::ShellOnly)
        .into_iter()
        .filter(|line| line.path != Path::new("postrm"))
        .filter_map(|line| {
            let command = need.unguarded(&line.text)?;
            Some((line.path, line.number, command.to_string()))
        })
        .collect();
    for (path, number, command) in hits {
        ctx.diagnostic(
            Diagnostic::warn(format!("{command} (does not satisfy {})", need.package()))
                .with_location(Location {
                    path,
                    line: Some(number),
                    column: None,
                }),
        );
    }
}
