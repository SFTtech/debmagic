use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::maintainer_script::{Continuations, interpreter, maintainer_script_lines};
use crate::lint::rule::BinaryPackageRule;

static X11_FONT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^usr/share/fonts/X11/.*\.(?:afm|pcf|pfa|pfb)(?:\.gz)?$").expect("valid regex")
});

/// lintian's `update-fonts-*` call: a command position (line start, a
/// backtick, `&`, `;`, `(`, `|`, `{`, `if`, `then`, `do`, `while`, `!`, or
/// after `env` assignments), then the optional `/usr/bin/` and one argument.
static UPDATE_FONTS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?:(?:^|[`&;(|{])\s*|(?:if|then|do|while|!)\s+|env(?:\s+[[:alnum:]_]+=(?:\S+|"[^"]*"|'[^']*'))*\s+)(?:/usr/bin/)?update-fonts-(?:alias|dir|scale)\s\S+"#,
    )
    .expect("valid regex")
});

pub struct MissingCallToUpdateFonts;

declare_rule! {
    /// A Binary package ships an X11 font but its `postinst` never calls
    /// `update-fonts-*`.
    ///
    /// One Diagnostic per Data files entry under `usr/share/fonts/X11/` ending
    /// in `.afm`, `.pcf`, `.pfa`, or `.pfb` (optionally `.gz`), directories
    /// excluded. Only when `postinst` is a regular file with a shebang
    /// interpreter, and no code line of it calls `update-fonts-alias`,
    /// `update-fonts-dir`, or `update-fonts-scale` with an argument. Extra is
    /// the font path; the `postinst` is the Location.
    MissingCallToUpdateFonts,
    code = "LN0844",
    tag = "missing-call-to-update-fonts",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MissingCallToUpdateFonts {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(data) = ctx.data_files() else {
            return;
        };
        let mut fonts: Vec<String> = data
            .entries()
            .iter()
            .filter(|entry| entry.kind() != EntryKind::Directory)
            .filter_map(|entry| entry.path().to_str())
            .filter(|path| X11_FONT.is_match(path))
            .map(str::to_string)
            .collect();
        if fonts.is_empty() {
            return;
        }
        fonts.sort();
        let Some(control) = ctx.control_files() else {
            return;
        };
        let postinst = Path::new("postinst");
        let Some(entry) = control.get(postinst) else {
            return;
        };
        if entry.kind() != EntryKind::RegularFile {
            return;
        }
        let Ok(bytes) = control.read(entry) else {
            return;
        };
        if interpreter(&String::from_utf8_lossy(&bytes)).is_none() {
            return;
        }
        let called = maintainer_script_lines(control, Continuations::Always)
            .iter()
            .filter(|line| line.path.as_path() == postinst)
            .any(|line| UPDATE_FONTS.is_match(&line.text));
        if called {
            return;
        }
        let path: PathBuf = postinst.to_path_buf();
        for font in fonts {
            ctx.diagnostic(Diagnostic::warn(font).with_location(Location {
                path: path.clone(),
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

    const FONT: &str = "usr/share/fonts/X11/Type1/foo.afm";

    fn with_postinst(font: &str, postinst: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(font.to_string(), TestFile::new("font\n"))])
            .control_file("postinst", TestFile::new(postinst))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            with_postinst(
                FONT,
                "#!/bin/sh\nupdate-fonts-dir /usr/share/fonts/X11/Type1\n",
            ),
            with_postinst(
                FONT,
                "#!/bin/sh\nif true; then /usr/bin/update-fonts-scale /x; fi\n",
            ),
            with_postinst(FONT, "#!/bin/sh\nupdate-fonts-dir\\\n /x\n"),
            with_postinst(FONT, "#!/bin/sh\nenv LANG=C update-fonts-alias /x\n"),
            // No shebang: lintian has no interpreter and skips the script.
            with_postinst(FONT, "echo hi\n"),
            // Only postinst is checked.
            BinaryPackageCase::from(vec![(FONT.to_string(), TestFile::new("font\n"))])
                .control_file("preinst", TestFile::new("#!/bin/sh\necho hi\n")),
            // Not an X11 font.
            with_postinst("usr/share/fonts/truetype/foo.ttf", "#!/bin/sh\necho hi\n"),
        ];
        let fail = vec![
            with_postinst(FONT, "#!/bin/sh\necho hi\n"),
            with_postinst(
                "usr/share/fonts/X11/Type1/foo.pcf.gz",
                "#!/bin/sh\necho update-fonts-dir /x\n",
            ),
            with_postinst(FONT, "#!/bin/sh\n# update-fonts-dir /x\n"),
            with_postinst(FONT, "#!/bin/sh\nupdate-fonts-dir\n"),
            with_postinst(
                "usr/share/fonts/X11/Type1/bar.pfb",
                "#!/bin/sh\n\necho hi\n",
            ),
            BinaryPackageCase::from(vec![
                (FONT.to_string(), TestFile::new("font\n")),
                (
                    "usr/share/fonts/X11/Type1/baz.pfa".to_string(),
                    TestFile::new("font\n"),
                ),
            ])
            .control_file("postinst", TestFile::new("#!/bin/sh\n")),
        ];
        BinaryPackageTester::new(MissingCallToUpdateFonts, pass, fail).test_and_snapshot();
    }
}
