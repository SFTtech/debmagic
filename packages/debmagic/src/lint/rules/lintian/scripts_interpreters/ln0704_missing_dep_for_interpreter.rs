use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::interpreter::{self, Hashbang, InterpreterVersion, KnownInterpreter};
use crate::lint::package_relation::PackageRelation;
use crate::lint::rule::BinaryPackageRule;

pub struct MissingDepForInterpreter;

declare_rule! {
    /// An installed script's interpreter is not among the package's
    /// prerequisites.
    ///
    /// Executable regular Data files that start with `#!`. The interpreter is
    /// the first word of that line after a leading `/usr/bin/env`; its
    /// basename is looked up in lintian's interpreter table (`nodejs` falls
    /// back to `node`). The table's relation must be implied by `Pre-Depends`,
    /// `Depends`, `Recommends`, `Suggests`, `Provides`, and the package name.
    /// A family name such as `pike` needs any listed version; a versioned
    /// name such as `pike7.6` needs that version. Extra is the interpreter
    /// and `(does not satisfy <relation>)`; the path is the Location.
    ///
    /// Paths under `usr/share/doc/`, `usr/src/`, and
    /// `usr/share/cargo/registry/` are skipped. Other Tags cover `php…`,
    /// `python3`, `ruby`, `mawk`, `gawk`, `wish…`, `tclsh…`, and versioned
    /// `python…` and `ruby…` names. `csh` under `etc/csh/login.d/` and `fish`
    /// under `etc/fish.d/` are initialization files. `ocamlrun` and `escript`
    /// also accept an ABI-versioned virtual package. Udebs are checked too.
    MissingDepForInterpreter,
    code = "LN0704",
    tag = "missing-dep-for-interpreter",
    default_selected = true,
    experimental = false,
}

static OCAML_ABI: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^ocaml(?:-base)?(?:-nox)?-\d\.[\d.]+").expect("valid regex"));
static ERLANG_ABI: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^erlang-abi-[\d+\.]+$").expect("valid regex"));

impl BinaryPackageRule for MissingDepForInterpreter {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let package_name = ctx.package_name().to_string();
        let prerequisites = all_prerequisites(ctx.installation_control().fields(), &package_name);
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<_> = scripts(files)
            .into_iter()
            .filter_map(|script| missing(&script, &prerequisites).map(|extra| (script.path, extra)))
            .collect();
        for (path, extra) in hits {
            ctx.diagnostic(Diagnostic::error(extra).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

struct Script {
    path: PathBuf,
    name: String,
    interpreter: String,
}

/// Executable regular files with a hashbang, outside the skipped trees.
fn scripts(files: &FileTree) -> Vec<Script> {
    files
        .query()
        .kinds([EntryKind::RegularFile])
        .into_iter()
        .filter(|entry| entry.is_executable_file())
        .filter_map(|entry| {
            let name = entry.path().to_str()?.to_string();
            if name.starts_with("usr/share/doc/")
                || name.starts_with("usr/src/")
                || name.starts_with("usr/share/cargo/registry/")
            {
                return None;
            }
            let bytes = files.read(entry).ok()?;
            let interpreter = Hashbang::read(&bytes)?.interpreter();
            Some(Script {
                path: entry.path().to_path_buf(),
                name,
                interpreter,
            })
        })
        .collect()
}

/// lintian's `all_prerequisites`: `relation('all')` AND `Provides` AND the
/// package's own name.
fn all_prerequisites(fields: &ControlParagraph, package_name: &str) -> PackageRelation {
    fields
        .relations_and(&["Pre-Depends", "Depends", "Recommends", "Suggests"])
        .and(&fields.relation("Provides").unwrap_or_default())
        .and(&PackageRelation::parse(package_name))
}

/// The extra when this Tag fires for `script`.
fn missing(script: &Script, prerequisites: &PackageRelation) -> Option<String> {
    let basename = interpreter::basename(&script.interpreter);
    let known = known(&script.interpreter, basename)?;
    if known.prerequisites.is_empty()
        || other_tag(&known, basename, &script.name, prerequisites)
        || prerequisites.satisfies(&known.prerequisites)
    {
        return None;
    }
    Some(format!(
        "{} (does not satisfy {})",
        script.interpreter, known.prerequisites
    ))
}

/// The table entry, or lintian's `nodejs` fallback to `node`. lintian's
/// `php…` fallback only leads to `php-script-but-no-php-cli-dep`.
fn known(interpreter: &str, basename: &str) -> Option<KnownInterpreter> {
    interpreter::lookup(basename).or_else(|| {
        (interpreter == "nodejs")
            .then(|| interpreter::unversioned("node"))
            .flatten()
    })
}

/// Cases lintian reports under another Tag or not at all.
fn other_tag(
    known: &KnownInterpreter,
    basename: &str,
    name: &str,
    prerequisites: &PackageRelation,
) -> bool {
    let matches = |pattern: &Regex| {
        prerequisites
            .alternatives()
            .any(|alternative| pattern.is_match(alternative.package()))
    };
    match &known.version {
        InterpreterVersion::Unversioned => {
            basename.starts_with("php")
                || matches!(basename, "python3" | "ruby" | "mawk" | "gawk")
                || (basename == "csh" && name.starts_with("etc/csh/login.d/"))
                || (basename == "fish" && name.starts_with("etc/fish.d/"))
                || (basename == "ocamlrun" && matches(&OCAML_ABI))
                || (basename == "escript" && matches(&ERLANG_ABI))
        }
        InterpreterVersion::AnyVersion => {
            basename.starts_with("wish") || basename.starts_with("tclsh")
        }
        InterpreterVersion::Version(_) => {
            basename.starts_with("python") || basename.starts_with("ruby")
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn script(path: &str, content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::new(content).mode(0o755))])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            // Essential interpreters need no prerequisite.
            script("usr/bin/tool", "#!/bin/sh\n"),
            script("usr/bin/tool", "#!/usr/bin/perl\n"),
            // Satisfied by Depends, Recommends, Provides, or the package name.
            script("usr/bin/tool", "#!/usr/bin/lefty\n").field("Depends", "graphviz"),
            script("usr/bin/tool", "#!/usr/bin/wish\n").field("Recommends", "tk8.4 | wish"),
            script("usr/bin/tool", "#!/usr/bin/make -f\n").field("Depends", "build-essential"),
            script("usr/bin/tool", "#!/usr/bin/pike7.6\n").field("Suggests", "pike7.6-core"),
            script("usr/bin/tool", "#!/usr/bin/pike\n").field("Pre-Depends", "pike7.8"),
            script("usr/bin/tool", "#!/usr/bin/gnuplot\n").field("Provides", "gnuplot"),
            script("usr/bin/tool", "#!/usr/bin/gnuplot\n").package("gnuplot"),
            script("usr/bin/tool", "#!/usr/bin/run\n").field("Depends", "runit (>= 2.1.2-23)"),
            // Not executable, not a script, or not a regular file.
            BinaryPackageCase::from(vec![("usr/bin/tool", TestFile::new("#!/usr/bin/lefty\n"))]),
            BinaryPackageCase::from(vec![(
                "usr/bin/tool",
                TestFile::new("#!/usr/bin/lefty\n").mode(0o755).hardlink(),
            )]),
            script("usr/bin/tool", "lefty\n"),
            script("usr/bin/tool", "#!#!#!/usr/bin/lefty\n"),
            script("usr/bin/tool", "#!\n"),
            // Skipped trees.
            script("usr/share/doc/pkg/examples/tool", "#!/usr/bin/lefty\n"),
            script("usr/src/pkg/tool", "#!/usr/bin/lefty\n"),
            script("usr/share/cargo/registry/pkg/tool", "#!/usr/bin/lefty\n"),
            // Unknown interpreters are other Tags.
            script("usr/bin/tool", "#!/usr/bin/lintian\n"),
            script("usr/bin/tool", "#!/usr/bin/env python3.7\n"),
            script("usr/local/bin/tool", "#!/usr/local/bin/nodejs\n"),
            // Reported under other Tags.
            script("usr/bin/tool", "#!/usr/bin/php\n"),
            script("usr/bin/tool", "#!/usr/bin/php7.0\n"),
            script("usr/bin/tool", "#!/usr/bin/python3\n"),
            script("usr/bin/tool", "#!/usr/bin/env ruby\n"),
            script("usr/bin/tool", "#!/usr/bin/gawk -f\n"),
            script("usr/bin/tool", "#!/usr/bin/tclsh\n"),
            script("usr/bin/tool", "#!/usr/bin/ruby1.8\n"),
            // Initialization files and ABI-versioned virtual packages.
            script("etc/csh/login.d/tool", "#! /bin/csh\n"),
            script("etc/fish.d/tool", "#! /usr/bin/fish\n"),
            script("usr/bin/tool", "#!/usr/bin/ocamlrun\n")
                .field("Depends", "ocaml-base-nox-4.14.0"),
            script("usr/bin/tool", "#!/usr/bin/escript\n").field("Depends", "erlang-abi-17.1"),
        ];
        let fail = vec![
            script("usr/bin/lefty-foo", "#!/usr/local/bin/lefty\n").field("Depends", "test"),
            script("usr/bin/unversioned-pike", "#!/usr/bin/pike\n"),
            script("usr/bin/versioned-pike", "#!/usr/bin/pike7.6\n").field("Depends", "pike7.8"),
            script("usr/bin/jruby-broken", "#!/usr/bin/jruby\n"),
            script("usr/bin/usr-bin-env-nodejs", "#!/usr/bin/env nodejs\n"),
            script("usr/bin/usr-bin-env-node", "#!/usr/bin/env node\n"),
            script("usr/bin/tool", "#!/usr/bin/guile-3.0 -s\n").field("Depends", "guile"),
            script("usr/bin/tool", "#!/usr/bin/run\n").field("Depends", "runit (>= 2.1)"),
            script("usr/bin/tool", "#! /bin/csh\n"),
            // Only the family name of `wish` is another Tag.
            script("usr/bin/tool", "#!/usr/bin/wish8.6\n"),
            script("usr/bin/tool", "#!/usr/bin/ocamlrun\n"),
            script("usr/share/doc-base/tool", "#!/usr/bin/expect\n"),
            script("usr/lib/cgi-bin/tool", "#!/usr/bin/env lua5.4\n").udeb(),
        ];
        BinaryPackageTester::new(MissingDepForInterpreter, pass, fail).test_and_snapshot();
    }
}
