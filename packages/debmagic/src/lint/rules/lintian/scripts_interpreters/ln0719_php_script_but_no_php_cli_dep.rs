use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::interpreter;
use crate::lint::package_relation::PackageRelation;
use crate::lint::rule::BinaryPackageRule;

use super::ln0704_missing_dep_for_interpreter::{Script, all_prerequisites, scripts};

pub struct PhpScriptButNoPhpCliDep;

declare_rule! {
    /// A PHP script's package does not provide `php-cli`.
    ///
    /// Executable regular Data files whose interpreter basename starts with
    /// `php`, such as `/usr/bin/php`, `/usr/bin/php7.0`, or `php-cgi`. The
    /// relation is `php-cli:any` from lintian's `scripts/interpreters` table
    /// (`php` entry), and must be implied by `Pre-Depends`, `Depends`,
    /// `Recommends`, `Suggests`, `Provides`, and the package name. Extra is
    /// the interpreter and `(does not satisfy php-cli:any)`; the path is the
    /// Location. Paths under `usr/share/doc/`, `usr/src/`, and
    /// `usr/share/cargo/registry/` are skipped. A `php` interpreter in
    /// `/usr/local/` is still checked, but other `php…` names there are not.
    /// Udebs are checked too.
    PhpScriptButNoPhpCliDep,
    code = "LN0719",
    tag = "php-script-but-no-php-cli-dep",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PhpScriptButNoPhpCliDep {
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

/// The extra when this Tag fires for `script`.
fn missing(script: &Script, prerequisites: &PackageRelation) -> Option<String> {
    let basename = interpreter::basename(&script.interpreter);
    if !basename.starts_with("php") {
        return None;
    }
    let known = match interpreter::lookup(basename) {
        Some(known) => known,
        // lintian's `php…` fallback to the `php` entry. Not for `/usr/local/`.
        None if script.interpreter.starts_with("/usr/local/") => return None,
        None => interpreter::unversioned("php")?,
    };
    if known.prerequisites.is_empty() || prerequisites.satisfies(&known.prerequisites) {
        return None;
    }
    Some(format!(
        "{} (does not satisfy {})",
        script.interpreter, known.prerequisites
    ))
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
            // Satisfied by Depends, Recommends, Suggests, Provides, or the package name.
            script("usr/bin/tool", "#!/usr/bin/php\n").field("Depends", "php-cli"),
            script("usr/bin/tool", "#!/usr/bin/env php\n").field("Depends", "php-cli (>= 8)"),
            script("usr/bin/tool", "#!/usr/bin/php7.0\n").field("Suggests", "php-cli"),
            script("usr/bin/tool", "#!/usr/bin/php\n").field("Provides", "php-cli"),
            script("usr/bin/tool", "#!/usr/bin/php\n").package("php-cli"),
            // Not executable, not a script, or not a regular file.
            BinaryPackageCase::from(vec![("usr/bin/tool", TestFile::new("#!/usr/bin/php\n"))]),
            script("usr/bin/tool", "php\n"),
            script("usr/bin/tool", "#!#!#!/usr/bin/php\n"),
            // Skipped trees.
            script("usr/share/doc/pkg/examples/tool", "#!/usr/bin/php\n"),
            script("usr/src/pkg/tool", "#!/usr/bin/php\n"),
            script("usr/share/cargo/registry/pkg/tool", "#!/usr/bin/php\n"),
            // Other interpreters and php names under /usr/local/ are not checked.
            script("usr/local/bin/tool", "#!/usr/local/bin/php7.0\n"),
            script("usr/bin/tool", "#!/usr/bin/python3\n"),
            script("usr/bin/tool", "#!/usr/bin/env perl\n"),
        ];
        let fail = vec![
            script("usr/bin/php-script", "#!/usr/bin/php\n"),
            script("usr/bin/php-script", "#!/usr/bin/env php\n"),
            script("usr/bin/php-script", "#!/usr/bin/php7.0\n"),
            script("usr/bin/php-script", "#!/usr/bin/php -f\n"),
            script("usr/bin/php-script", "#!/usr/bin/env php7.0 -q\n"),
            script("usr/bin/php-script", "#!/usr/bin/php-cgi\n"),
            script("usr/local/bin/php-script", "#!/usr/local/bin/php\n"),
            script("usr/bin/php-script", "#!/usr/bin/php\n").field("Depends", "php-cgi"),
            script("usr/bin/php-script", "#!/usr/bin/php\n").field("Depends", "php7.0-cli"),
            script("usr/bin/php-script", "#!/usr/bin/php\n").package("php-cgi"),
            script("usr/bin/php-script", "#!/usr/bin/php\n").udeb(),
            // An alternative that may be another package does not imply php-cli.
            script("usr/bin/php-script", "#!/usr/bin/php\n")
                .field("Recommends", "php-cli | php8.2-cli"),
        ];
        BinaryPackageTester::new(PhpScriptButNoPhpCliDep, pass, fail).test_and_snapshot();
    }
}
