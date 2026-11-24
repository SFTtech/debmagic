use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::flag::flag_name;
use super::names::NameHit;
use crate::lint::rule::Severity;

pub struct FileNameInPathIsNotAscii;

declare_rule! {
    /// A file directly in a PATH directory has a non-ASCII name.
    ///
    /// PATH directories are `bin/`, `sbin/`, `usr/bin/`, `usr/sbin/`, and
    /// `usr/games/`. The final component must contain a byte above 127.
    /// The same name elsewhere is not flagged. Udebs are checked too. Extra
    /// is empty; the path is the Location.
    FileNameInPathIsNotAscii,
    code = "LN0077",
    tag = "file-name-in-PATH-is-not-ASCII",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for FileNameInPathIsNotAscii {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        flag_name(ctx, NameHit::PathNotAscii, Severity::Error);
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn tree(paths: &[&str]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            paths
                .iter()
                .map(|path| (path.to_string(), TestFile::new("x")))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(&[]),
            tree(&["usr/bin/tool", "usr/share/foo/鳥", "bin/ascii"]),
        ];
        let fail = vec![
            tree(&["usr/bin/鳥の詩"]),
            tree(&["usr/sbin/é", "usr/games/ñ"]),
            tree(&["bin/é"]).udeb(),
        ];
        BinaryPackageTester::new(FileNameInPathIsNotAscii, pass, fail).test_and_snapshot();
    }
}
