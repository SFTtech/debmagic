use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::dh_scan::gated_scan;
use super::rules_text::read_utf8;

pub struct DhQuiltAddonButQuiltSourceFormat;

declare_rule! {
    /// `dh` loads the `quilt` addon while the source format is `3.0 (quilt)`.
    ///
    /// Scans `debian/rules` the way lintian's `debhelper` Check does: `\`
    /// continuations are joined first, then comment lines are dropped. For an
    /// indented `dh` sequencer line, every `--with` list entry that is `quilt`
    /// reports when the source `Format` is exactly `3.0 (quilt)`. The format is
    /// the `Format` field of the `.dsc` of a Source package, else the first
    /// line of `debian/source/format`. Only reached after a dh command has
    /// been seen; a `Build-Driver` other than `debian-rules` is not scanned.
    /// Extra is `dh ... --with` and the entry as written. Location is
    /// `debian/rules` and the line. Applies to the Source tree and Source
    /// package.
    DhQuiltAddonButQuiltSourceFormat,
    code = "LN0522",
    tag = "dh-quilt-addon-but-quilt-source-format",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DhQuiltAddonButQuiltSourceFormat {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some((_, scan)) = gated_scan(ctx) else {
            return;
        };
        if scan.quilt_addons.is_empty() {
            return;
        }
        let format = match ctx.dsc() {
            Some(dsc) => dsc.fields().get("Format").map(str::to_string),
            None => ctx.files().and_then(|files| {
                let text = read_utf8(files, "debian/source/format")?;
                Some(text.lines().next().unwrap_or("").trim().to_string())
            }),
        };
        if format.as_deref() != Some("3.0 (quilt)") {
            return;
        }
        for (position, addon) in scan.quilt_addons {
            ctx.diagnostic(
                Diagnostic::warn(format!("dh ... --with {addon}")).with_location(Location {
                    path: PathBuf::from("debian/rules"),
                    line: Some(position),
                    column: None,
                }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "\
Source: example\n\
Maintainer: Example <ex@example.com>\n\
Standards-Version: 4.7.2\n\
\n\
Package: example\n\
Architecture: all\n\
Description: example\n extra\n";

    fn package(rules: &str, format: Option<&str>) -> Vec<(&'static str, TestFile)> {
        let mut files = vec![
            ("debian/control", TestFile::new(CONTROL)),
            ("debian/rules", TestFile::new(rules)),
        ];
        if let Some(format) = format {
            files.push(("debian/source/format", TestFile::new(format)));
        }
        files
    }

    const QUILT: Option<&str> = Some("3.0 (quilt)\n");

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no rules"))],
            package("%:\n\tdh $@ --with quilt\n", None),
            package("%:\n\tdh $@ --with quilt\n", Some("3.0 (native)\n")),
            package("%:\n\tdh $@ --with python3\n", QUILT),
            package("%:\n\tdh $@ --with quilt2\n", QUILT),
            package("%:\n\t# dh $@ --with quilt\n", QUILT),
            // Not indented, so not the sequencer.
            package("dh $@ --with quilt\n%:\n\tdh_clean\n", QUILT),
            // The addon list is only read on a `dh` line.
            package("%:\n\tdh $@\n\tfoo --with quilt\n", QUILT),
        ];
        let fail = vec![
            package("%:\n\tdh $@ --with quilt\n", QUILT),
            package("%:\n\tdh $@ --with=python3,quilt\n", QUILT),
            package("%:\n\tdh $@ --with 'quilt,python3'\n", QUILT),
            package("%:\n\tdh $@ --with quilt --with=quilt\n", QUILT),
            package("%:\n\tdh $@ \\\n\t  --with quilt\n", QUILT),
        ];
        SourceTreeTester::new(DhQuiltAddonButQuiltSourceFormat, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(package(
            "%:\n\tdh $@ --with quilt\n",
            Some("3.0 (native)\n"),
        ))];
        let fail = vec![SourcePackageCase::from(package(
            "%:\n\tdh $@ --with quilt\n",
            QUILT,
        ))];
        SourcePackageTester::new(DhQuiltAddonButQuiltSourceFormat, pass, fail).test_and_snapshot();
    }
}
