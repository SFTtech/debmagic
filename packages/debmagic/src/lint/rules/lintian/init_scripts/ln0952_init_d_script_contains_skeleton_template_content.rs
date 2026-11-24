use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::init_script;

pub struct InitDScriptContainsSkeletonTemplateContent;

declare_rule! {
    /// An `etc/init.d` script contains text from the `skeleton` example.
    ///
    /// A line containing `Please remove the "Author" lines` or `Example
    /// initscript` is reported, at its line, anywhere in the line, comments
    /// included. Lines of the first LSB section (from `### BEGIN INIT INFO`
    /// through `### END INIT INFO`, or the first non-comment line) are not
    /// read, since lintian consumes them while parsing the header. The scripts
    /// checked are the ones the init-d check reads, so
    /// an unregistered `README`, `skeleton`, `rc`, or `rcS` is skipped unless
    /// `postinst` registers it. `initscripts` and `sysvinit` are skipped. The
    /// extra is empty and the script path with its line is the Location.
    /// Symlinked scripts are not read here.
    InitDScriptContainsSkeletonTemplateContent,
    code = "LN0952",
    tag = "init.d-script-contains-skeleton-template-content",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for InitDScriptContainsSkeletonTemplateContent {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let package = ctx.package_name().to_string();
        let registered = ctx
            .control_files()
            .map(init_script::registered_names)
            .unwrap_or_default();
        let Some(data) = ctx.data_files() else {
            return;
        };
        let hits: Vec<(std::path::PathBuf, u32)> =
            init_script::init_d_scripts(data, &registered, &package)
                .into_iter()
                .flat_map(|script| {
                    let path = script.path;
                    init_script::body_lines(&script.text)
                        .into_iter()
                        .filter(|(_, line)| SKELETON.is_match(line))
                        .map(|(line, _)| (path.clone(), line))
                        .collect::<Vec<_>>()
                })
                .collect();
        for (path, line) in hits {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: Some(line),
                column: None,
            }));
        }
    }
}

static SKELETON: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"Please remove the "Author" lines|Example initscript"#).expect("valid regex")
});

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn script(name: &str, body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(format!("etc/init.d/{name}"), TestFile::new(body))])
    }

    fn registered(name: &str, body: &str) -> BinaryPackageCase {
        script(name, body).control_file(
            "postinst",
            TestFile::new(format!("#!/bin/sh\nupdate-rc.d {name} defaults\n")),
        )
    }

    #[test]
    fn binary() {
        let clean = "#!/bin/sh\n# Provides: foo\ncase \"$1\" in start) ;; esac\n";
        let skeleton = "#!/bin/sh\n# Example initscript\n# Please remove the \"Author\" lines\n";
        let pass = vec![
            script("foo", clean),
            script("skeleton", skeleton),
            script("README", skeleton),
            script("foo", skeleton).package("initscripts"),
            script("foo", skeleton).package("sysvinit"),
            script("foo", skeleton).udeb().package("initscripts"),
            script(
                "foo",
                "#!/bin/sh\n### BEGIN INIT INFO\n# Short-Description: Example initscript\n### END INIT INFO\n",
            ),
        ];
        let fail = vec![
            registered("skeleton", skeleton),
            script("foo", "#!/bin/sh\n# Example initscript\n"),
            script("foo", "#!/bin/sh\n# Please remove the \"Author\" lines\n"),
            script("foo", skeleton),
            script("foo", "#!/bin/sh\necho Example initscript\n").udeb(),
        ];
        BinaryPackageTester::new(InitDScriptContainsSkeletonTemplateContent, pass, fail)
            .test_and_snapshot();
    }
}
