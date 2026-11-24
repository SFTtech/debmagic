use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::flag::flag_name;
use super::names::NameHit;

pub struct ShippedFileWithoutUtf8Name;

declare_rule! {
    /// An installed path is not valid UTF-8.
    ///
    /// Any Data files entry, directories included, whose stored name is not
    /// UTF-8. A name directly in a PATH directory (`bin/`, `sbin/`,
    /// `usr/bin/`, `usr/sbin/`, `usr/games/`) is left to
    /// `file-name-in-PATH-is-not-ASCII`. Udebs are checked too. Extra is
    /// empty; the raw path is the Location, printed as its bytes read as
    /// Latin-1, as lintian does.
    ShippedFileWithoutUtf8Name,
    code = "LN0702",
    tag = "shipped-file-without-utf8-name",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for ShippedFileWithoutUtf8Name {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        flag_name(ctx, NameHit::NotUtf8, Severity::Error);
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn tree(paths: &[&[u8]]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            paths
                .iter()
                .map(|path| (OsString::from_vec(path.to_vec()), TestFile::new("foo\n")))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(&[]),
            tree(&["usr/share/doc/filenames/bokmål".as_bytes()]),
            tree(&["usr/share/doc/filenames/鳥の詩".as_bytes()]),
            // Directly in a PATH directory: file-name-in-PATH-is-not-ASCII.
            tree(&[b"usr/bin/bokm\xe5l", b"sbin/\xff"]),
        ];
        let fail = vec![
            tree(&[b"usr/share/foo/bokm\xe5l"]),
            tree(&[
                b"usr/share/doc/filenames/bokm\xe5l",
                b"usr/share/doc/filenames/bokm\\\xe5l",
            ]),
            tree(&[b"usr/bin/sub/\xff"]),
            BinaryPackageCase::from(vec![(
                OsString::from_vec(b"usr/share/\xe9t\xe9".to_vec()),
                TestFile::directory(),
            )]),
            tree(&[b"usr/lib/\xc3"]).udeb(),
        ];
        BinaryPackageTester::new(ShippedFileWithoutUtf8Name, pass, fail).test_and_snapshot();
    }
}
