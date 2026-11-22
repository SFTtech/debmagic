use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::pear::{CHANNEL_XML, PearSource};

pub struct PearChannelWithoutPkgPhpToolsBuilddep;

declare_rule! {
    /// The Source has a PEAR `channel.xml`, but Build-Depends does not satisfy
    /// `pkg-php-tools`. Only `pkg-php-tools` counts here, not
    /// `dh-sequence-phppear`.
    ///
    /// Only when a `.php` file is shipped. The Diagnostic has no extra and
    /// `channel.xml` as Location.
    PearChannelWithoutPkgPhpToolsBuilddep,
    code = "LN1284",
    tag = "pear-channel-without-pkg-php-tools-builddep",
    default_selected = true,
    experimental = false,
}

impl SourceRule for PearChannelWithoutPkgPhpToolsBuilddep {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(pear) = PearSource::new(ctx) else {
            return;
        };
        if pear.channel_xml && !pear.has_pkg_php_tools() {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path: PathBuf::from(CHANNEL_XML),
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestCase, TestFile};

    use super::*;

    fn tree(build_depends: &str, channel: bool) -> TestCase {
        let mut files = vec![
            (
                "debian/control",
                TestFile::new(format!(
                    "Source: foo\nBuild-Depends: {build_depends}\n\nPackage: foo\nArchitecture: all\nDescription: foo\n extra\n"
                )),
            ),
            ("foo.php", TestFile::new("<?php\n")),
        ];
        if channel {
            files.push(("channel.xml", TestFile::new("<channel/>\n")));
        }
        TestCase::source_tree(files)
    }

    #[test]
    fn source() {
        let pass = vec![
            tree("debhelper-compat (= 13), pkg-php-tools", true),
            // No channel.xml, so no channel to build.
            tree("debhelper-compat (= 13)", false),
            // dh-sequence-phppear is not pkg-php-tools.
            tree("debhelper-compat (= 13), dh-sequence-phppear", false),
        ];
        let fail = vec![
            tree("debhelper-compat (= 13)", true),
            tree("debhelper-compat (= 13), dh-sequence-phppear", true),
        ];
        SourceTreeTester::new(PearChannelWithoutPkgPhpToolsBuilddep, pass, fail)
            .test_and_snapshot();
    }
}
