use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::debhelper::DebhelperFacts;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::group_context::GroupContext;
use crate::lint::rule::GroupRule;

pub struct PackageUsesDhRunitButLacksBreaksSubstvar;

declare_rule! {
    /// A package that ships `etc/sv/` files from `dh_runit` and depends on
    /// `runit` has no `Breaks` that satisfies `${runit:Breaks}`.
    ///
    /// The source package counts as using `dh_runit` when `debian/rules` runs
    /// the `dh_runit` command or `dh ... --with runit` (a `dh` line with a
    /// make variable could name any add-on, so it counts too). Each deb
    /// (not udeb) of the Group that strongly depends on `runit` (Pre-Depends
    /// or Depends), has a file or directory under `etc/sv/`, and lacks the
    /// `Breaks` is reported; the extra is its name and the Location is
    /// `debian/rules`. Nothing is reported when the compat level is 14 or
    /// higher, or when the source build-depends on a `dh-sequence-debputy`
    /// virtual package (`dh-sequence-zz-debputy`, `dh-sequence-zz-debputy-rrr`
    /// too), as these add the substitution variable themselves.
    ///
    /// A Group Rule: it needs the installables' files, so it runs once per
    /// Group that has a `.dsc` and the Diagnostic attaches to the Source
    /// package. Lintian runs it as part of its `debhelper` Check, in the
    /// Source package; there it also sees the installables of its Group.
    PackageUsesDhRunitButLacksBreaksSubstvar,
    code = "LN1500",
    tag = "package-uses-dh-runit-but-lacks-breaks-substvar",
    default_selected = true,
    experimental = false,
}

/// lintian's `$DH_LEVEL_MISC_DEPENDS_AUTO_APPLY`: debhelper adds `${misc:...}`
/// variables, and with them `${runit:Breaks}`, by itself.
const MISC_DEPENDS_AUTO_APPLY: u32 = 14;

impl GroupRule for PackageUsesDhRunitButLacksBreaksSubstvar {
    fn run(&self, ctx: &mut GroupContext<'_>) {
        let control = ctx.debian_control().cloned();
        let Some(files) = ctx.patched_files() else {
            return;
        };
        let Some(control) = control else {
            return;
        };
        let facts = DebhelperFacts::from_tree(Some(&control), files);
        let uses_runit = facts.has_dynamic_addons()
            || facts
                .commands()
                .iter()
                .any(|command| command.name() == "dh_runit")
            || facts.addons().iter().any(|addon| addon == "runit");
        if !uses_runit || facts.level() >= MISC_DEPENDS_AUTO_APPLY {
            return;
        }
        let build_prerequisites = control.source().relations_and(&[
            "Build-Depends",
            "Build-Depends-Indep",
            "Build-Depends-Arch",
        ]);
        if [
            "dh-sequence-debputy",
            "dh-sequence-zz-debputy",
            "dh-sequence-zz-debputy-rrr",
        ]
        .iter()
        .any(|name| build_prerequisites.satisfies(name))
        {
            return;
        }

        for installable in ctx.installables() {
            if installable.is_udeb() {
                continue;
            }
            // The relations are those of `debian/control`, as written with
            // their substitution variables, not those of the built package.
            let Some(declared) = control
                .installables()
                .iter()
                .find(|declared| declared.name() == installable.name())
            else {
                continue;
            };
            let fields = declared.fields();
            if !fields
                .relations_and(&["Pre-Depends", "Depends"])
                .satisfies("runit")
                || fields
                    .relation("Breaks")
                    .unwrap_or_default()
                    .satisfies("${runit:Breaks}")
            {
                continue;
            }
            let Some(data) = ctx.data_files(&installable) else {
                continue;
            };
            if !has_service_directory_file(data) {
                continue;
            }
            ctx.diagnostic(
                Diagnostic::warn(installable.name()).with_location(Location {
                    path: PathBuf::from("debian/rules"),
                    line: None,
                    column: None,
                }),
            );
        }
    }
}

/// An entry under `etc/sv/`, or the directory itself (lintian lists
/// directories with a trailing slash, which its `^etc/sv/` matches).
fn has_service_directory_file(files: &FileTree) -> bool {
    files.entries().iter().any(|entry| {
        let path = entry.path().as_os_str().as_bytes();
        path.starts_with(b"etc/sv/") || (path == b"etc/sv" && entry.kind() == EntryKind::Directory)
    })
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{GroupCase, GroupTester, TestFile};

    use super::*;

    const DSC: &str = "Source: example\nVersion: 1.0\n";
    const RULES: &str = "#!/usr/bin/make -f\n%:\n\tdh $@ --with runit\n";
    const COMPAT13: &str = "debhelper-compat (= 13), dh-runit";

    #[derive(Clone, Copy)]
    enum Files {
        None,
        Service,
        ServiceDirectory,
        Other,
    }

    /// One package: name, relations in `debian/control`, files, udeb.
    type Package = (&'static str, &'static str, Files, bool);

    fn group(rules: &str, build_depends: &str, packages: &[Package]) -> GroupCase {
        let mut control = format!("Source: example\nBuild-Depends: {build_depends}\n");
        for (name, relations, _, udeb) in packages {
            control.push_str(&format!(
                "\nPackage: {name}\nArchitecture: all\n{relations}{}Description: x\n x\n",
                if *udeb { "Package-Type: udeb\n" } else { "" }
            ));
        }
        let mut case = GroupCase::new(DSC).source_files([
            ("debian/rules", rules.to_string()),
            ("debian/control", control),
        ]);
        for (name, _, files, udeb) in packages {
            case = if *udeb {
                case.udeb(*name, "")
            } else {
                case.installable(*name, "")
            };
            let files: Vec<(&str, TestFile)> = match files {
                Files::None => vec![],
                Files::Service => vec![("etc/sv/example/run", TestFile::new("#!/bin/sh\n"))],
                Files::ServiceDirectory => vec![("etc/sv", TestFile::directory())],
                Files::Other => vec![("etc/svx/run", TestFile::new(""))],
            };
            case = case.files(files);
        }
        case
    }

    fn one(rules: &str, build_depends: &str, relations: &'static str, files: Files) -> GroupCase {
        group(rules, build_depends, &[("pkg-a", relations, files, false)])
    }

    #[test]
    fn group_rule() {
        let runit = "Depends: runit\n";
        let pass = vec![
            // The Breaks.
            one(
                RULES,
                COMPAT13,
                "Depends: runit\nBreaks: ${runit:Breaks}\n",
                Files::Service,
            ),
            // No dependency on runit, or only a weak one.
            one(RULES, COMPAT13, "", Files::Service),
            one(RULES, COMPAT13, "Recommends: runit\n", Files::Service),
            // No service files.
            one(RULES, COMPAT13, runit, Files::None),
            one(RULES, COMPAT13, runit, Files::Other),
            // A udeb.
            group(RULES, COMPAT13, &[("pkg-a", runit, Files::Service, true)]),
            // dh_runit is not run.
            one("%:\n\tdh $@\n", COMPAT13, runit, Files::Service),
            // Debhelper adds the variable at compat 14, debputy does too.
            one(
                RULES,
                "debhelper-compat (= 14), dh-runit",
                runit,
                Files::Service,
            ),
            one(
                RULES,
                "debhelper-compat (= 13), dh-sequence-debputy",
                runit,
                Files::Service,
            ),
            one(
                RULES,
                "debhelper-compat (= 13), dh-sequence-zz-debputy-rrr",
                runit,
                Files::Service,
            ),
            // No packaging tree to read.
            GroupCase::new(DSC)
                .installable("pkg-a", "Depends: runit\n")
                .files([("etc/sv/a/run", TestFile::new(""))]),
        ];
        let fail = vec![
            one(RULES, COMPAT13, runit, Files::Service),
            one(
                RULES,
                COMPAT13,
                "Pre-Depends: runit (>= 2.1)\nBreaks: other\n",
                Files::Service,
            ),
            // The command, not the add-on.
            one(
                "%:\n\tdh $@\noverride_dh_runit:\n\tdh_runit\n",
                COMPAT13,
                runit,
                Files::Service,
            ),
            // An add-on list from a variable could hold runit.
            one(
                "%:\n\tdh $@ --with $(ADDONS)\n",
                COMPAT13,
                runit,
                Files::Service,
            ),
            // A service directory alone is enough; packages are judged one by one.
            group(
                RULES,
                COMPAT13,
                &[
                    ("pkg-a", runit, Files::ServiceDirectory, false),
                    (
                        "b",
                        "Depends: runit\nBreaks: ${runit:Breaks}\n",
                        Files::Service,
                        false,
                    ),
                    ("pkg-c", runit, Files::Service, false),
                ],
            ),
        ];
        GroupTester::new(PackageUsesDhRunitButLacksBreaksSubstvar, pass, fail).test_and_snapshot();
    }
}
