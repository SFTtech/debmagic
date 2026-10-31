use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::{files, scripts};

pub struct PostinstUsesDbInput;

declare_rule! {
    /// `postinst` calls `db_input` and `config` does not.
    ///
    /// The match is the substring `db_input` on a script line, after comments
    /// are stripped. Only the first `postinst` hit is reported. `config` is
    /// scanned first; a `db_input` there suppresses the tag. The package must
    /// ship `config`, `templates`, or a debconf `preinst`, or the Check
    /// returns before maintainer scripts. Extra is empty. The Location is the
    /// Check's script position. Udebs are not checked.
    PostinstUsesDbInput,
    code = "LN0325",
    tag = "postinst-uses-db-input",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PostinstUsesDbInput {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let hit = {
            let Some(tree) = ctx.control_files() else {
                return;
            };
            if !files::gate(tree).open() {
                return;
            }
            let mut config_calls_db_input = false;
            let mut found = None;
            for script in scripts::scan(tree) {
                let name = script.path.file_name().and_then(|file| file.to_str());
                match name {
                    Some("config") => {
                        if script
                            .lines
                            .iter()
                            .any(|line| scripts::mentions_db_input(&line.text))
                        {
                            config_calls_db_input = true;
                        }
                    }
                    Some("postinst") if !config_calls_db_input => {
                        if let Some(line) = script
                            .lines
                            .iter()
                            .find(|line| scripts::mentions_db_input(&line.text))
                        {
                            found = Some((script.path.clone(), line.position));
                        }
                    }
                    _ => {}
                }
            }
            found
        };
        if let Some((path, line)) = hit {
            ctx.diagnostic(warn_at(path, line));
        }
    }
}

fn warn_at(path: PathBuf, line: u32) -> Diagnostic {
    Diagnostic::warn("").with_location(files::located(path, Some(line)))
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn with_config(config: &str, postinst: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file("config", TestFile::new(config))
            .control_file("postinst", TestFile::new(postinst))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            with_config(
                "#!/bin/sh\ndb_input low debconf/translated\n",
                "#!/bin/sh\ndb_input low debconf/translated\n",
            ),
            with_config("#!/bin/sh\necho db_input\n", "#!/bin/sh\ndb_input low x\n"),
            with_config("#!/bin/sh\n", "#!/bin/sh\n# db_input low x\ndb_get x\n"),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("postinst", TestFile::new("#!/bin/sh\ndb_input low x\n")),
            with_config("#!/bin/sh\n", "db_input low x\n"),
            with_config("#!/bin/sh\n", "#!/bin/sh\ndb_input low x\n").udeb(),
        ];
        let fail = vec![
            with_config(
                "#!/bin/sh\ndb_get x\n",
                "#!/bin/sh\ndb_input low debconf/translated\n",
            ),
            with_config("db_input low x\n", "#!/bin/sh\ndb_input low x\n"),
            with_config(
                "#!/bin/sh\n",
                "#!/bin/sh\necho db_input\ndb_input low again\n",
            ),
            with_config("#!/bin/sh\n", "#!/bin/sh\ndb_input \\\nlow foo\n"),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("templates", TestFile::new("Template: a/b\nType: string\n"))
                .control_file("postinst", TestFile::new("#!/bin/sh\ndb_input low x\n")),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file(
                    "preinst",
                    TestFile::new("#!/bin/sh\n. /usr/share/debconf/confmodule\n"),
                )
                .control_file("postinst", TestFile::new("#!/bin/sh\ndb_input medium x\n")),
        ];
        BinaryPackageTester::new(PostinstUsesDbInput, pass, fail).test_and_snapshot();
    }
}
