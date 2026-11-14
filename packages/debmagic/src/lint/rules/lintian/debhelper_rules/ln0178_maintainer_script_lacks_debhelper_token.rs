use std::path::{Component, Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_control::InstallablePackageType;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::rule::SourceRule;

use super::super::debian_rules::{complete_lines, read_rules_from_tree};
use super::commands::is_maint_command;
use super::scans_debian_rules;

pub struct MaintainerScriptLacksDebhelperToken;

declare_rule! {
    /// A packaging-tree maintainer script has no `#DEBHELPER#` token.
    ///
    /// After `debian/rules` runs the `dh` sequencer, or a debhelper command
    /// that rewrites maintainer scripts without `-n` surrounded by whitespace,
    /// each direct child of `debian/` named `postinst`, `preinst`, `postrm`,
    /// `prerm`, or `<package>.` plus one of those, must contain `#DEBHELPER#`.
    /// The `dh` sequencer sets this even when the line also passes `-n`. A
    /// prefixed script whose installable is a udeb is skipped, as is an
    /// unprefixed script when the source builds a single udeb. An unknown
    /// package name is not a udeb. Extra is empty. Location is the script
    /// path. Applies to the Source tree and Source package: these are
    /// `debian/` scripts in the packaging tree, not the binary control
    /// tarball. A missing rules file, or a `Build-Driver` other than
    /// `debian-rules`, does not emit.
    MaintainerScriptLacksDebhelperToken,
    code = "LN0178",
    tag = "maintainer-script-lacks-debhelper-token",
    default_selected = true,
    experimental = false,
}

static DH_COMMAND: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s+[@+-]?(?:\S+=\S+\s+)*(dh_\S+)").expect("valid regex"));
static DH_SEQUENCER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:\s+[@+-]?(?:\S+=\S+\s+)*)dh\s+").expect("valid regex"));
static SUPPRESSED_WITH_N: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s+-n\s+").expect("valid regex"));
static SCRIPT_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:(.*)\.)?(?:post|pre)(?:inst|rm)$").expect("valid regex"));

impl SourceRule for MaintainerScriptLacksDebhelperToken {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        if !scans_debian_rules(ctx.debian_control()) {
            return;
        }
        let Some(contents) = ctx.files().and_then(read_rules_from_tree) else {
            return;
        };
        if !modifies_scripts(&contents) {
            return;
        }
        let Some(control) = ctx.debian_control() else {
            // Borrow ends before `files`; a missing control simply is not a udeb.
            return emit_scripts(ctx, None);
        };
        let packages: Vec<(String, InstallablePackageType)> = control
            .installables()
            .iter()
            .map(|installable| (installable.name().to_string(), installable.package_type()))
            .collect();
        emit_scripts(ctx, Some(&packages));
    }
}

fn emit_scripts(
    ctx: &mut SourceContext<'_>,
    packages: Option<&[(String, InstallablePackageType)]>,
) {
    let Some(files) = ctx.files() else {
        return;
    };
    let candidates: Vec<PathBuf> = files
        .entries()
        .iter()
        .filter(|entry| {
            matches!(
                entry.kind(),
                EntryKind::RegularFile | EntryKind::Hardlink | EntryKind::Symlink
            )
        })
        .filter_map(|entry| {
            let package = script_package(entry.path())?;
            if is_udeb(packages, package.as_deref()) {
                return None;
            }
            Some(entry.path().to_path_buf())
        })
        .collect();
    let missing: Vec<PathBuf> = candidates
        .into_iter()
        .filter(|path| !read_text(files, path).contains("#DEBHELPER#"))
        .collect();
    for path in missing {
        ctx.diagnostic(Diagnostic::warn("").with_location(Location {
            path,
            line: None,
            column: None,
        }));
    }
}

/// `Some(None)` is an unprefixed script. `None` is not a maintainer script.
fn script_package(path: &Path) -> Option<Option<String>> {
    if !is_debian_child(path) {
        return None;
    }
    let name = path.file_name()?.to_str()?;
    let captures = SCRIPT_NAME.captures(name)?;
    Some(match captures.get(1) {
        Some(package) if !package.as_str().is_empty() => Some(package.as_str().to_string()),
        _ => None,
    })
}

fn is_debian_child(path: &Path) -> bool {
    let mut components = path.components();
    components.next() == Some(Component::Normal("debian".as_ref()))
        && components.next().is_some()
        && components.next().is_none()
}

fn is_udeb(packages: Option<&[(String, InstallablePackageType)]>, package: Option<&str>) -> bool {
    let Some(packages) = packages else {
        return false;
    };
    match package {
        Some(name) => packages.iter().any(|(installable, kind)| {
            installable == name && *kind == InstallablePackageType::Udeb
        }),
        None => packages.len() == 1 && packages[0].1 == InstallablePackageType::Udeb,
    }
}

fn modifies_scripts(contents: &str) -> bool {
    complete_lines(contents).into_iter().any(|(_, line)| {
        if let Some(captures) = DH_COMMAND.captures(&line) {
            let command = &captures[1];
            // `(?!autoreconf)` in lintian: `dh_autoreconf` is not a counted command.
            if command.starts_with("dh_autoreconf") {
                return false;
            }
            return is_maint_command(command) && !SUPPRESSED_WITH_N.is_match(&line);
        }
        DH_SEQUENCER.is_match(&line)
    })
}

/// File bytes, following a relative symlink. Invalid UTF-8 and unreadable
/// paths are empty, which is how lintian's `decoded_utf8` misses the token.
fn read_text(files: &FileTree, path: &Path) -> String {
    let mut path = path.to_path_buf();
    for _ in 0..8 {
        let Some(entry) = files.get(&path) else {
            return String::new();
        };
        match entry.kind() {
            EntryKind::Directory => return String::new(),
            EntryKind::RegularFile | EntryKind::Hardlink => {
                let Ok(bytes) = files.read(entry) else {
                    return String::new();
                };
                return String::from_utf8(bytes).unwrap_or_default();
            }
            EntryKind::Symlink => {
                let Some(target) = entry.link_target() else {
                    return String::new();
                };
                let Some(next) = resolve_link(&path, target) else {
                    return String::new();
                };
                path = next;
            }
        }
    }
    String::new()
}

fn resolve_link(from: &Path, target: &Path) -> Option<PathBuf> {
    if target.is_absolute() {
        return None;
    }
    let mut resolved = from.parent().unwrap_or(Path::new("")).to_path_buf();
    for component in target.components() {
        match component {
            Component::Normal(part) => resolved.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                if !resolved.pop() {
                    return None;
                }
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(resolved)
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

    const DH: &str = "#!/usr/bin/make -f\n%:\n\tdh $@\n";
    const BARE: &str = "#!/bin/sh\nset -e\nexit 0\n";
    const TOKEN: &str = "#!/bin/sh\nset -e\n#DEBHELPER#\nexit 0\n";

    fn tree(
        rules: &'static str,
        scripts: &[(&'static str, TestFile)],
    ) -> Vec<(&'static str, TestFile)> {
        let mut files = vec![
            ("debian/control", TestFile::new(CONTROL)),
            ("debian/rules", TestFile::new(rules)),
        ];
        for (path, file) in scripts {
            files.push((*path, file.clone()));
        }
        files
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            vec![
                ("debian/control", TestFile::new(CONTROL)),
                ("debian/postinst", TestFile::new(BARE)),
            ],
            tree(DH, &[("debian/postinst", TestFile::new(TOKEN))]),
            tree(DH, &[]),
            tree(
                "#!/usr/bin/make -f\nbuild:\n\tdh_installdeb -n \n",
                &[("debian/postinst", TestFile::new(BARE))],
            ),
            tree(
                "#!/usr/bin/make -f\nbuild:\n\tdh_installdeb -n -a\n",
                &[("debian/postinst", TestFile::new(BARE))],
            ),
            tree(
                "#!/usr/bin/make -f\nbuild:\n\tdh_clean\n",
                &[("debian/postinst", TestFile::new(BARE))],
            ),
            tree(
                "#!/usr/bin/make -f\nbuild:\n\ttrue\n",
                &[("debian/postinst", TestFile::new(BARE))],
            ),
            tree(
                "#\tdh $@\n\tdh_autoreconf\n",
                &[("debian/postinst", TestFile::new(BARE))],
            ),
            tree(DH, &[("debian/tests/postinst", TestFile::new(BARE))]),
            tree(DH, &[("debian/config", TestFile::new(BARE))]),
            tree(
                DH,
                &[("debian/postinst", TestFile::new("# comment #DEBHELPER#\n"))],
            ),
            vec![
                (
                    "debian/control",
                    TestFile::new(
                        "Source: example\n\
                         Build-Driver: debputy\n\
                         \n\
                         Package: example\n\
                         Architecture: all\n\
                         Description: example\n extra\n",
                    ),
                ),
                ("debian/rules", TestFile::new(DH)),
                ("debian/postinst", TestFile::new(BARE)),
            ],
            vec![
                (
                    "debian/control",
                    TestFile::new(
                        "Source: example\n\
                         \n\
                         Package: example-udeb\n\
                         Package-Type: udeb\n\
                         Architecture: any\n\
                         Description: udeb\n extra\n",
                    ),
                ),
                ("debian/rules", TestFile::new(DH)),
                ("debian/postinst", TestFile::new(BARE)),
                ("debian/example-udeb.postinst", TestFile::new(BARE)),
            ],
            tree(
                DH,
                &[
                    ("debian/body", TestFile::new(TOKEN)),
                    ("debian/postinst", TestFile::symlink("body")),
                ],
            ),
        ];
        let fail = vec![
            tree(DH, &[("debian/postinst", TestFile::new(BARE))]),
            tree(DH, &[("debian/example.postinst", TestFile::new(BARE))]),
            tree(
                DH,
                &[
                    ("debian/preinst", TestFile::new(BARE)),
                    ("debian/postinst", TestFile::new(BARE)),
                    ("debian/prerm", TestFile::new(BARE)),
                    ("debian/postrm", TestFile::new(BARE)),
                ],
            ),
            tree(
                "#!/usr/bin/make -f\nbuild:\n\tdh_installdeb\n",
                &[("debian/postinst", TestFile::new(BARE))],
            ),
            // `-n` at end of line has no trailing whitespace, so it still rewrites.
            tree(
                "#!/usr/bin/make -f\nbuild:\n\tdh_installdeb -n\n",
                &[("debian/postinst", TestFile::new(BARE))],
            ),
            tree(
                "#!/usr/bin/make -f\nbuild:\n\tdh_installdeb -n \n\tdh_installinit\n",
                &[("debian/postinst", TestFile::new(BARE))],
            ),
            tree(
                DH,
                &[("debian/postinst", TestFile::from_bytes(vec![0xff, 0xfe]))],
            ),
            tree(
                DH,
                &[
                    ("debian/body", TestFile::new(BARE)),
                    ("debian/postinst", TestFile::symlink("body")),
                ],
            ),
            vec![
                (
                    "debian/control",
                    TestFile::new(
                        "Source: example\n\
                         \n\
                         Package: example\n\
                         Architecture: all\n\
                         Description: example\n extra\n\
                         \n\
                         Package: example-udeb\n\
                         Package-Type: udeb\n\
                         Architecture: any\n\
                         Description: udeb\n extra\n",
                    ),
                ),
                ("debian/rules", TestFile::new(DH)),
                ("debian/postinst", TestFile::new(BARE)),
                ("debian/example.postinst", TestFile::new(BARE)),
                ("debian/example-udeb.postinst", TestFile::new(BARE)),
            ],
            vec![
                (
                    "debian/control",
                    TestFile::new(
                        "Source: example\n\
                         \n\
                         Package: example-udeb\n\
                         Package-Type: udeb\n\
                         Architecture: any\n\
                         Description: udeb\n extra\n",
                    ),
                ),
                ("debian/rules", TestFile::new(DH)),
                ("debian/other.postinst", TestFile::new(BARE)),
            ],
        ];
        SourceTreeTester::new(MaintainerScriptLacksDebhelperToken, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree(
            DH,
            &[("debian/postinst", TestFile::new(TOKEN))],
        ))];
        let fail = vec![SourcePackageCase::from(tree(
            DH,
            &[("debian/postinst", TestFile::new(BARE))],
        ))];
        SourcePackageTester::new(MaintainerScriptLacksDebhelperToken, pass, fail)
            .test_and_snapshot();
    }
}
