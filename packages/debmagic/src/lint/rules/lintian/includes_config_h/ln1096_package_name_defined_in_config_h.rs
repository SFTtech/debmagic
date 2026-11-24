use std::sync::LazyLock;

use regex::bytes::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct PackageNameDefinedInConfigH;

declare_rule! {
    /// A header `usr/include/.../config.h` of the Binary package uses the
    /// identifier `PACKAGE_NAME`, which autoconf's own `config.h` also
    /// defines, so the header clashes with autoconf-based packages.
    ///
    /// Walks the Data files regular files and hardlinks whose path starts
    /// with `usr/include/` and ends with `/config.h` (any byte stands for the
    /// `.`, as in lintian). The file's bytes must contain `PACKAGE_NAME` as a
    /// whole ASCII word. Extra is the path, as lintian prints it; a path that
    /// is not UTF-8 is shown lossily. There is no Location: the path is the
    /// message. Udebs are checked too.
    PackageNameDefinedInConfigH,
    code = "LN1096",
    tag = "package-name-defined-in-config-h",
    default_selected = true,
    experimental = false,
}

static INCLUDE_DIR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?-u)^usr/include/").expect("valid regex"));

static CONFIG_H: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?-u)/config.h$").expect("valid regex"));

static PACKAGE_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?-u)\bPACKAGE_NAME\b").expect("valid regex"));

impl BinaryPackageRule for PackageNameDefinedInConfigH {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut hits = Vec::new();
        for entry in files.entries() {
            if !entry.is_file() {
                continue;
            }
            let name = entry.path().as_os_str().as_encoded_bytes();
            if !(INCLUDE_DIR.is_match(name) && CONFIG_H.is_match(name)) {
                continue;
            }
            // An unreadable file has no bytes to judge, as lintian's
            // `bytes` would fail on it: skip it.
            let Ok(bytes) = files.read(entry) else {
                continue;
            };
            if PACKAGE_NAME.is_match(&bytes) {
                hits.push(entry.path().to_string_lossy().into_owned());
            }
        }
        for path in hits {
            ctx.diagnostic(Diagnostic::warn(path));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    const DEFINES: &str = "#define PACKAGE_NAME \"example\"\n";

    fn case(path: &str, content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path.to_string(), TestFile::new(content))])
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            case("usr/include/example/config.h", "#define FOO 1\n"),
            // Not under `usr/include/`.
            case("usr/share/example/config.h", DEFINES),
            case("etc/config.h", DEFINES),
            // The name must end in `/config.h`.
            case("usr/include/example/xconfig.h", DEFINES),
            case("usr/include/example/config.hpp", DEFINES),
            case("usr/include/config.h.in", DEFINES),
            // The identifier is a whole word only.
            case(
                "usr/include/example/config.h",
                "#define MY_PACKAGE_NAME 1\n#define PACKAGE_NAMES 2\n",
            ),
            // Not a file.
            BinaryPackageCase::from(vec![(
                "usr/include/example/config.h",
                TestFile::symlink("/usr/include/other/config.h"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/include/example/config.h",
                TestFile::directory(),
            )]),
        ];
        let fail = vec![
            case("usr/include/custom/config.h", DEFINES),
            case("usr/include/config.h", DEFINES),
            case("usr/include/example/config.h", "PACKAGE_NAME"),
            // A hardlink is a file for lintian.
            BinaryPackageCase::from(vec![
                (
                    "usr/include/example/config.h",
                    TestFile::new(DEFINES).hardlink_to("usr/share/example/copy.h"),
                ),
                ("usr/share/example/copy.h", TestFile::new(DEFINES)),
            ]),
            // Udebs are checked too.
            case("usr/include/example/config.h", DEFINES).udeb(),
            // A name that is not UTF-8 is still matched by its bytes.
            BinaryPackageCase::from(vec![(
                OsString::from_vec(b"usr/include/bokm\xe5l/config.h".to_vec()),
                TestFile::from_bytes(DEFINES.as_bytes()),
            )]),
        ];
        BinaryPackageTester::new(PackageNameDefinedInConfigH, pass, fail).test_and_snapshot();
    }
}
