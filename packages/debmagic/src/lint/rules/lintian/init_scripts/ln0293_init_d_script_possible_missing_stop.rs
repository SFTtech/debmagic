use std::collections::HashSet;
use std::path::Path;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::init_script;

pub struct InitDScriptPossibleMissingStop;

declare_rule! {
    /// An `etc/init.d` script stops in some of runlevels 0, 1, and 6, but
    /// not all of them.
    ///
    /// Valid `Default-Stop` tokens other than `s`, `S`, and `2`–`5` are the
    /// shutdown set (`0`, `1`, and `6`). The finding is skipped when that set
    /// is empty or complete, when the only lowercased `Default-Start` token
    /// is `s`, and for scripts named `killprocs`, `sendsigs`, `halt`, or
    /// `reboot`. Extra is the missing levels in the order `0`, `1`, `6`. The
    /// script path is the Location. The same scripts are skipped as for the
    /// required-option Tag.
    InitDScriptPossibleMissingStop,
    code = "LN0293",
    tag = "init.d-script-possible-missing-stop",
    default_selected = true,
    experimental = false,
}

const EXEMPT: &[&str] = &["killprocs", "sendsigs", "halt", "reboot"];
const REQUIRED: &[&str] = &["0", "1", "6"];

impl BinaryPackageRule for InitDScriptPossibleMissingStop {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for script in scripts(ctx) {
            let name = script
                .path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("");
            if EXEMPT.contains(&name) {
                continue;
            }
            let starts = init_script::start_keys(&script.lsb.default_start);
            if starts.len() == 1 && starts.contains("s") {
                continue;
            }
            let stop: HashSet<&str> = script
                .lsb
                .default_stop
                .iter()
                .filter(|level| matches!(level.as_str(), "0" | "1" | "6"))
                .map(String::as_str)
                .collect();
            if !REQUIRED.iter().any(|level| stop.contains(level)) {
                continue;
            }
            let missing: Vec<&str> = REQUIRED
                .iter()
                .copied()
                .filter(|level| !stop.contains(level))
                .collect();
            if missing.is_empty() {
                continue;
            }
            ctx.diagnostic(Diagnostic::warn(missing.join(" ")).with_location(at(&script.path)));
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

    fn lsb(start: &str, stop: &str) -> String {
        format!(
            "#!/bin/sh\n\
             ### BEGIN INIT INFO\n\
             # Default-Start: {start}\n\
             # Default-Stop: {stop}\n\
             ### END INIT INFO\n"
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            script("complete", &lsb("2 3 4 5", "0 1 6")),
            script("none", &lsb("2 3 4 5", "S 2 3 4 5")),
            script("only-s", &lsb("S", "0 6")),
            script("both-letters", &lsb("S s", "0")),
            script("halt", &lsb("2 3 4 5", "0 6")),
            script("killprocs", &lsb("2", "1")),
            script("sendsigs", &lsb("2", "1")),
            script("reboot", &lsb("2", "1")),
            script("skeleton", &lsb("2 3 5", "0 6")),
            script("pkg", &lsb("2 3 5", "0 6")).package("initscripts"),
        ];
        let fail = vec![
            script("missing-one", &lsb("2 3 5", "0 6")),
            script("parsing", &lsb("0", "0")),
            script("with-start", &lsb("S 2", "0")),
            script("only-one", &lsb("2 3 4 5", "1")),
            script("udeb-script", &lsb("2", "6")).udeb(),
        ];
        BinaryPackageTester::new(InitDScriptPossibleMissingStop, pass, fail).test_and_snapshot();
    }
}
