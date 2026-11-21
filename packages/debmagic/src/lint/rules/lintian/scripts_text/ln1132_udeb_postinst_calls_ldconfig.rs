use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct UdebPostinstCallsLdconfig;

declare_rule! {
    /// A udeb's `postinst` calls `ldconfig`.
    ///
    /// One hit per Control file named exactly `postinst` in a udeb whose text
    /// has `ldconfig` as a word before any `#` on a line. The text must be
    /// valid UTF-8; otherwise the file is skipped, as lintian's empty
    /// `decoded_utf8` does. No shebang is required, and a `\` does not join
    /// lines. Extra is empty; the Location is the file with no line number.
    /// Other maintainer scripts and deb packages are reported by
    /// `maintscript-calls-ldconfig` instead, not by this Rule.
    UdebPostinstCallsLdconfig,
    code = "LN1132",
    tag = "udeb-postinst-calls-ldconfig",
    default_selected = true,
    experimental = false,
}

static MAINTAINER_SCRIPT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^config|(?:pre|post)(?:inst|rm)$").expect("valid regex"));
static LDCONFIG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^[^#]*\bldconfig\b").expect("valid regex"));

impl BinaryPackageRule for UdebPostinstCallsLdconfig {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if !ctx.is_udeb() {
            return;
        }
        let Some(files) = ctx.control_files() else {
            return;
        };
        let hits: Vec<_> = files
            .query()
            .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
            .path(&MAINTAINER_SCRIPT)
            .into_iter()
            .filter(|entry| entry.path() == Path::new("postinst"))
            .filter_map(|entry| {
                let bytes = files.read(entry).ok()?;
                let text = std::str::from_utf8(&bytes).ok()?;
                LDCONFIG.is_match(text).then(|| entry.path().to_path_buf())
            })
            .collect();
        for path in hits {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn control(name: &str, content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file(name, TestFile::new(content))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).udeb(),
            // Not a udeb: reported by `maintscript-calls-ldconfig`.
            control("postinst", "#!/bin/sh\nldconfig\n"),
            control("postinst", "#!/bin/sh\n# ldconfig\n").udeb(),
            control("postinst", "#!/bin/sh\necho foo # ldconfig\n").udeb(),
            control("postinst", "#!/bin/sh\nnotldconfig\n").udeb(),
            control("postinst", "#!/bin/sh\nld\\\nconfig\n").udeb(),
            control("postrm", "#!/bin/sh\nldconfig\n").udeb(),
            control("prerm", "#!/bin/sh\nldconfig\n").udeb(),
            control("triggers", "#!/bin/sh\nldconfig\n").udeb(),
            control("my-postinst", "#!/bin/sh\nldconfig\n").udeb(),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("postinst", TestFile::symlink("foo"))
                .udeb(),
            // Invalid UTF-8 reads as empty.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file(
                    "postinst",
                    TestFile::from_bytes(b"\xff\nldconfig\n".to_vec()),
                )
                .udeb(),
            BinaryPackageCase::from(vec![("usr/lib/example.so", TestFile::new("ldconfig"))]).udeb(),
        ];
        let fail = vec![
            control("postinst", "#!/bin/sh\nldconfig\n").udeb(),
            control("postinst", "#!/bin/sh\nset -e\n\nldconfig\n\n#DEBHELPER#\n").udeb(),
            control("postinst", "ldconfig\n").udeb(),
            control("postinst", "#!/bin/sh\nfoo; ldconfig\n").udeb(),
            control("postinst", "#!/bin/sh\nldconfig -n /usr/lib\n").udeb(),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file(
                    "postinst",
                    TestFile::new("#!/bin/sh\nldconfig\n").hardlink(),
                )
                .udeb(),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("postinst", TestFile::new("#!/bin/sh\nldconfig\n"))
                .control_file("postrm", TestFile::new("#!/bin/sh\nldconfig\n"))
                .udeb(),
        ];
        BinaryPackageTester::new(UdebPostinstCallsLdconfig, pass, fail).test_and_snapshot();
    }
}
