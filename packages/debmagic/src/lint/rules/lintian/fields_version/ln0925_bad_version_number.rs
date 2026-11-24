use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct BadVersionNumber;

declare_rule! {
    /// The `Version` of the Binary package fails the syntax dpkg requires.
    ///
    /// Reads the Installation control and reports the unfolded version as
    /// extra. The version is well formed when an optional numeric epoch is
    /// followed by an upstream part that starts with a digit and holds only
    /// letters, digits and `.+~:-`, and an optional revision after the last
    /// hyphen holds only letters, digits and `.+~`. An empty version is
    /// reported too. Udebs are checked like other Binary packages. Only the
    /// Binary package subject is covered; lintian also checks source versions.
    BadVersionNumber,
    code = "LN0925",
    tag = "bad-version-number",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for BadVersionNumber {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(version) = ctx
            .installation_control()
            .fields()
            .unfolded_value("Version")
        else {
            return;
        };
        if !is_valid_version(&version) {
            ctx.diagnostic(Diagnostic::error(version));
        }
    }
}

/// Dpkg's syntax check for a version string.
pub(super) fn is_valid_version(version: &str) -> bool {
    let rest = match version.split_once(':') {
        Some((epoch, rest)) => {
            if epoch.is_empty() || !epoch.bytes().all(|byte| byte.is_ascii_digit()) {
                return false;
            }
            rest
        }
        None => version,
    };
    let upstream = match rest.rsplit_once('-') {
        Some((upstream, revision)) => {
            let revision_ok = !revision.is_empty()
                && revision
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '~'));
            if !revision_ok {
                return false;
            }
            upstream
        }
        None => rest,
    };
    upstream.starts_with(|c: char| c.is_ascii_digit())
        && upstream
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '~' | ':' | '-'))
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn version(value: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Version", value)
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            version("1.0"),
            version("1:2.3-4"),
            version("1.0-1ubuntu1"),
            version("2.0~rc1-1+b1"),
            version("2.0.1-1.2"),
            version("1.0").udeb(),
        ];
        let fail = vec![
            version("1.0?"),
            version("1.0?").udeb(),
            version("a1.0"),
            version(":1.0"),
            version("x:1.0"),
            version("1.0-"),
            version("1.0-x_y"),
            version("1.0-1-2:"),
            version(""),
        ];
        BinaryPackageTester::new(BadVersionNumber, pass, fail).test_and_snapshot();
    }
}
