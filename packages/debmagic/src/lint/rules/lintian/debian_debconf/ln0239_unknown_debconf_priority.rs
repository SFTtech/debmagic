use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::{files, scripts};

pub struct UnknownDebconfPriority;

declare_rule! {
    /// `db_input` or `db_text` uses a priority other than `low`, `medium`,
    /// `high`, or `critical`.
    ///
    /// A `$variable` priority, including one written in quotes, is ignored.
    /// The comparison is case-sensitive. Extra is the priority. The Location
    /// is the Check's script position. Udebs are not checked.
    UnknownDebconfPriority,
    code = "LN0239",
    tag = "unknown-debconf-priority",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for UnknownDebconfPriority {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let hits = {
            let Some(files) = ctx.control_files() else {
                return;
            };
            if !files::gate(files).open() {
                return;
            }
            let mut hits = Vec::new();
            for script in scripts::scan(files) {
                for line in &script.lines {
                    let Some(call) = scripts::db_invocation(&line.text) else {
                        continue;
                    };
                    if scripts::shell_priority(&call.priority)
                        || scripts::valid_priority(&call.priority)
                    {
                        continue;
                    }
                    hits.push((script.path.clone(), line.position, call.priority));
                }
            }
            hits
        };
        for (path, line, priority) in hits {
            ctx.diagnostic(
                Diagnostic::error(priority).with_location(files::located(path, Some(line))),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn config(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file("config", TestFile::new(body))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            config(
                "#!/bin/sh\ndb_input low a/b\ndb_text medium a/b\ndb_input high a/b\ndb_input critical a/b\n",
            ),
            config("#!/bin/sh\ndb_input $foo a/b\ndb_input \"$foo\" a/b\ndb_input 'medium' a/b\n"),
            config("#!/bin/sh\ndb_text \\\nhigh a/b\n"),
            config("db_input LOW a/b\n"),
            config("#!/bin/sh\ndb_input LOW a/b\n").udeb(),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("postinst", TestFile::new("#!/bin/sh\ndb_input LOW a/b\n")),
        ];
        let fail = vec![
            config("#!/bin/sh\ndb_text LOW debconf/test\ndb_input normal debconf/test\n"),
            config("#!/bin/sh\ndb_input unknown debconf/unknown-priority\n"),
            config("#!/bin/sh\ndb_input \"normal\" debconf/test\n"),
            config("#!/bin/sh\ndb_input \\\nunknown foo\n"),
        ];
        BinaryPackageTester::new(UnknownDebconfPriority, pass, fail).test_and_snapshot();
    }
}
