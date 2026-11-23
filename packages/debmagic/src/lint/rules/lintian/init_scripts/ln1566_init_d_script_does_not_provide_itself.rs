use std::path::Path;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::init_script;

pub struct InitDScriptDoesNotProvideItself;

declare_rule! {
    /// An `etc/init.d` script declares `Provides` but none of its facilities
    /// is the script's own name.
    ///
    /// The keyword is read from the first LSB section, as the other init.d
    /// Rules read it. A script whose section has no `Provides` keyword is not
    /// checked; an empty `Provides:` is present, has no facility, and is
    /// reported. The last `Provides` wins. A facility matches the
    /// basename when it is equal, or when the basename is the facility followed
    /// by one character and `sh`, as lintian's `^\Q$facility\E(?:.sh)?$` reads
    /// (its `.` is any character, so `foo_sh` matches `foo`). Extra is empty;
    /// the script is the Location. Severity is info and the Tag is not
    /// selected by default, as in lintian.
    InitDScriptDoesNotProvideItself,
    code = "LN1566",
    tag = "init.d-script-does-not-provide-itself",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for InitDScriptDoesNotProvideItself {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for script in scripts(ctx) {
            let provides_defined = script
                .section
                .keywords
                .iter()
                .any(|(_, keyword)| keyword == "provides");
            if !provides_defined {
                continue;
            }
            let basename = script
                .path
                .file_name()
                .map(|name| name.as_encoded_bytes())
                .unwrap_or_default();
            let provides_itself = script
                .lsb
                .provides
                .iter()
                .any(|facility| provides_itself(basename, facility));
            if !provides_itself {
                ctx.diagnostic(
                    Diagnostic::warn("")
                        .with_location(at(&script.path))
                        .with_severity(Severity::Info),
                );
            }
        }
    }
}

fn scripts(ctx: &mut BinaryPackageContext<'_>) -> Vec<init_script::InitDScript> {
    let package = ctx.package_name().to_string();
    let registered = {
        let Some(control) = ctx.control_files() else {
            return Vec::new();
        };
        init_script::registered_names(control)
    };
    let Some(data) = ctx.data_files() else {
        return Vec::new();
    };
    init_script::init_d_scripts(data, &registered, &package)
}

/// `^\Q$facility\E(?:.sh)?$` on the basename: equal, or the facility, one
/// character (not a newline) and `sh`.
fn provides_itself(basename: &[u8], facility: &str) -> bool {
    let Some(rest) = basename.strip_prefix(facility.as_bytes()) else {
        return false;
    };
    if rest.is_empty() {
        return true;
    }
    let Some(any) = rest.strip_suffix(b"sh") else {
        return false;
    };
    match std::str::from_utf8(any) {
        Ok(text) => text.chars().count() == 1 && text != "\n",
        Err(_) => any.len() == 1 && any[0] != b'\n',
    }
}

fn at(path: &Path) -> Location {
    Location {
        path: path.to_path_buf(),
        line: None,
        column: None,
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn script(name: &str, body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(format!("etc/init.d/{name}"), TestFile::new(body))])
    }

    fn lsb(provides: &str) -> String {
        format!(
            "#!/bin/sh\n### BEGIN INIT INFO\n# Provides: {provides}\n### END INIT INFO\nexit 0\n"
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            script("foo", &lsb("foo")),
            script("foo.sh", &lsb("foo")),
            // `.` in the pattern is any one character.
            script("foo_sh", &lsb("foo")),
            script("foo", &lsb("bar foo")),
            // The last Provides of the first section wins.
            script(
                "foo",
                "#!/bin/sh\n### BEGIN INIT INFO\n# Provides: bar\n# Provides: foo\n### END INIT INFO\n",
            ),
            // No Provides keyword, and no LSB section at all.
            script(
                "foo",
                "#!/bin/sh\n### BEGIN INIT INFO\n# Default-Start: 2\n### END INIT INFO\n",
            ),
            script("foo", "#!/bin/sh\n# Provides: bar\n"),
            script("foo", &lsb("bar")).package("sysvinit"),
        ];
        let fail = vec![
            script("foo", &lsb("bar")),
            script("foo", &lsb("")),
            script("foo", &lsb("foo.sh")),
            script("foo", &lsb("$virtual")),
            // `sh` alone is not one character before it.
            script("foosh", &lsb("foo")),
            script("foo", &lsb("foo_sh")),
            script("foo", &lsb("bar")).udeb(),
            script("foo", &lsb("foo.sh bar")),
        ];
        BinaryPackageTester::new(InitDScriptDoesNotProvideItself, pass, fail).test_and_snapshot();
    }
}
