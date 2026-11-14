use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::{files, scripts};

pub struct PostinstDoesNotLoadConfmodule;

declare_rule! {
    /// `postinst` does not load a debconf confmodule while `config` exists.
    ///
    /// Loading is `.` or `source` of `/usr/share/debconf/confmodule`, `use`
    /// or `require` of `Debconf::Client::ConfModule`, or an obsolete module
    /// name. A file without `#!` is not a script and is not reported. When
    /// the script is present, the message is empty and the Location is the
    /// file with no line. When `postinst` is missing, the message and
    /// Location are empty: the Check passes the script name, and the pinned
    /// recipe records no extra. Udebs are not checked.
    PostinstDoesNotLoadConfmodule,
    code = "LN0324",
    tag = "postinst-does-not-load-confmodule",
    default_selected = true,
    experimental = false,
}

enum Hit {
    Script(PathBuf),
    Missing,
}

impl BinaryPackageRule for PostinstDoesNotLoadConfmodule {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let hit = {
            let Some(tree) = ctx.control_files() else {
                return;
            };
            if !files::gate(tree).config {
                return;
            }
            let mut loaded = false;
            let mut script_path = None;
            for script in scripts::scan(tree) {
                if script
                    .path
                    .file_name()
                    .is_none_or(|file| file != "postinst")
                {
                    continue;
                }
                script_path = Some(script.path.clone());
                loaded = script
                    .lines
                    .iter()
                    .any(|line| scripts::loads_confmodule(&line.text));
            }
            if loaded {
                None
            } else if let Some(path) = script_path {
                Some(Hit::Script(path))
            } else if !files::is_present(tree, "postinst") {
                Some(Hit::Missing)
            } else {
                None
            }
        };
        match hit {
            Some(Hit::Script(path)) => {
                ctx.diagnostic(Diagnostic::warn("").with_location(files::located(path, None)));
            }
            Some(Hit::Missing) => ctx.diagnostic(Diagnostic::warn("")),
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn config_and_postinst(postinst: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file("config", TestFile::new("#!/bin/sh\n"))
            .control_file("postinst", TestFile::new(postinst))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            config_and_postinst("#!/bin/sh\n. /usr/share/debconf/confmodule\n"),
            config_and_postinst("#!/bin/sh\nsource /usr/share/debconf/confmodule\n"),
            config_and_postinst("#!/usr/bin/perl\nuse Debconf::Client::ConfModule;\n"),
            config_and_postinst("#!/usr/bin/perl\nrequire Debconf::Client::ConfModule;\n"),
            config_and_postinst("#!/bin/sh\n. /usr/share/debconf/confmodule.sh\n"),
            config_and_postinst("#!/usr/bin/perl\nuse Debian::DebConf::Client::ConfModule;\n"),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("config", TestFile::new("#!/bin/sh\n"))
                .control_file("postinst", TestFile::new("db_input low x\n")),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("templates", TestFile::new("Template: a/b\nType: string\n"))
                .control_file("postinst", TestFile::new("#!/bin/sh\n")),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("config", TestFile::new("#!/bin/sh\n"))
                .udeb(),
        ];
        let fail = vec![
            config_and_postinst("#!/bin/sh\n\nset -e\n\ndb_input low debconf/translated\n"),
            config_and_postinst("#!/bin/sh\n# . /usr/share/debconf/confmodule\n"),
            config_and_postinst("#!/bin/sh\n/usr/share/debconf/confmodule\n"),
            config_and_postinst("#!/bin/sh\n. \"/usr/share/debconf/confmodule\"\n"),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("config", TestFile::new("#!/bin/sh\n")),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("config", TestFile::new("#!/bin/sh\n"))
                .control_file("postinst", TestFile::symlink("missing")),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("config", TestFile::new("#!/bin/sh\n"))
                .control_file("postinst", TestFile::new("#!/bin/sh\n").hardlink()),
        ];
        BinaryPackageTester::new(PostinstDoesNotLoadConfmodule, pass, fail).test_and_snapshot();
    }
}
