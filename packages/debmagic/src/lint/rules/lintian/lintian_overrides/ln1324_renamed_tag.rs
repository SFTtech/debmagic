use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::lintian_tags::current_tag_name;
use crate::lint::rule::{BinaryPackageRule, Severity};

pub struct RenamedTag;

declare_rule! {
    /// A Lintian override names a Tag that lintian renamed.
    ///
    /// One Diagnostic per override in the installed override file
    /// (`usr/share/lintian/overrides/<package>`, else `.gz`) whose Tag is an old
    /// name that maps to a different current name. Extra is the old name, then
    /// ` => `, then the current name. The override file and line are the
    /// Location. A name lintian does not know at all is `alien-tag`, not this.
    /// Pedantic, selected only when asked for. Overrides are not applied.
    RenamedTag,
    code = "LN1324",
    tag = "renamed-tag",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for RenamedTag {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(overrides) = ctx.lintian_overrides() else {
            return;
        };
        let path = PathBuf::from(overrides.path());
        let renamed: Vec<(String, &'static str, u32)> = overrides
            .overrides()
            .iter()
            .filter_map(|entry| {
                let current = current_tag_name(entry.tag())?;
                (current != entry.tag()).then(|| (entry.tag().to_string(), current, entry.line()))
            })
            .collect();
        for (old, current, line) in renamed {
            ctx.diagnostic(
                Diagnostic::warn(format!("{old} => {current}"))
                    .with_severity(Severity::Pedantic)
                    .with_location(Location {
                        path: path.clone(),
                        line: Some(line),
                        column: None,
                    }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn installed(content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(
            "usr/share/lintian/overrides/example".to_string(),
            TestFile::new(content),
        )])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            installed("example: package-installs-apt-sources\n"),
            installed("example: no-copyright-file\n"),
            // Not known at all: alien-tag.
            installed("example: mystery-tag-that-is-not-real\n"),
        ];
        let fail = vec![
            installed("example: package-install-apt-sources\n"),
            installed("# why\n\nexample [amd64]: manpage-has-bad-whatis-entry\n"),
        ];
        BinaryPackageTester::new(RenamedTag, pass, fail).test_and_snapshot();
    }
}
