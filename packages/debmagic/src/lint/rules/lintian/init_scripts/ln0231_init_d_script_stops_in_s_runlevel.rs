use std::path::Path;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::init_script;

pub struct InitDScriptStopsInSRunlevel;

declare_rule! {
    /// An `etc/init.d` script lists `S` or `s` in LSB `Default-Stop`.
    ///
    /// The keyword is read from the first `### BEGIN INIT INFO` section. The
    /// last `Default-Stop` line there wins, and the value is split on
    /// whitespace. One finding per `S` or `s` token. The same scripts are
    /// skipped as for the required-option Tag. Extra is empty. The script
    /// path is the Location.
    InitDScriptStopsInSRunlevel,
    code = "LN0231",
    tag = "init-d-script-stops-in-s-runlevel",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for InitDScriptStopsInSRunlevel {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let package = ctx.package_name().to_string();
        let registered = {
            let Some(control) = ctx.control_files() else {
                return;
            };
            init_script::registered_names(control)
        };
        let Some(data) = ctx.data_files() else {
            return;
        };
        let scripts = init_script::init_d_scripts(data, &registered, &package);
        for script in scripts {
            for _ in 0..script.stops_in_s {
                ctx.diagnostic(Diagnostic::warn("").with_location(at(&script.path)));
            }
        }
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

    fn lsb(stop: &str) -> String {
        format!(
            "#!/bin/sh\n\
             ### BEGIN INIT INFO\n\
             # Default-Stop:      {stop}\n\
             ### END INIT INFO\n\
             exit 0\n"
        )
    }

    #[test]
    fn test() {
        let last_wins = "\
#!/bin/sh
### BEGIN INIT INFO
# Default-Stop:      S
# Default-Stop:      0 1 6
### END INIT INFO
";
        let ignored_second = "\
#!/bin/sh
### BEGIN INIT INFO
# Default-Stop:      0 1 6
### END INIT INFO
### BEGIN INIT INFO
# Default-Stop:      S
### END INIT INFO
";
        let two_spaces = "\
#!/bin/sh
### BEGIN INIT INFO
#  Default-Stop:      S
### END INIT INFO
";
        let pass = vec![
            script("ok", &lsb("0 1 6")),
            script("outside", "#!/bin/sh\n# Default-Stop: S\nexit 0\n"),
            script("last-wins", last_wins),
            script("second-ignored", ignored_second),
            script("two-spaces", two_spaces),
            script("skeleton", &lsb("S")),
            script("bad-level", &lsb("X")),
            script("pkg", &lsb("S")).package("initscripts"),
            script("pkg", &lsb("S")).package("sysvinit"),
        ];
        let unterminated = "\
#!/bin/sh
### BEGIN INIT INFO
# Default-Stop:      S 0 1 6
echo not a comment
";
        let both = lsb("S s");
        let fail = vec![
            script("capital", &lsb("S")),
            script("lower", &lsb("s")),
            script("with-numbers", &lsb("S 0 1 6")),
            script("both", &both),
            script("unterminated", unterminated),
            script("skeleton", &lsb("S")).control_file(
                "postinst",
                TestFile::new("#!/bin/sh\nupdate-rc.d skeleton defaults\n"),
            ),
            script("udeb-script", &lsb("S")).udeb(),
        ];
        BinaryPackageTester::new(InitDScriptStopsInSRunlevel, pass, fail).test_and_snapshot();
    }
}
