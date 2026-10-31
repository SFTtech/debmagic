use std::collections::HashSet;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::{files, scripts, templates};

pub struct PossibleDebconfNoteAbuse;

declare_rule! {
    /// `db_input` or `db_text` shows a `note` or `text` template at `low` or
    /// `medium` priority.
    ///
    /// A `$variable` priority is ignored. `low` is allowed when the previous
    /// physical line contains `/dev/`. Extra is the template name as written
    /// in the script. The Location is the Check's script position. Udebs are
    /// not checked.
    PossibleDebconfNoteAbuse,
    code = "LN0237",
    tag = "possible-debconf-note-abuse",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PossibleDebconfNoteAbuse {
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
            let abusive = note_or_text(files);
            let mut hits = Vec::new();
            for script in scripts::scan(files) {
                for line in &script.lines {
                    let Some(call) = scripts::db_invocation(&line.text) else {
                        continue;
                    };
                    if scripts::shell_priority(&call.priority) || !abusive.contains(&call.template)
                    {
                        continue;
                    }
                    let low_or_medium = call.priority == "low" || call.priority == "medium";
                    let device_exception = call.priority == "low"
                        && script.mentions_dev(line.position.saturating_sub(1));
                    if low_or_medium && !device_exception {
                        hits.push((script.path.clone(), line.position, call.template));
                    }
                }
            }
            hits
        };
        for (path, line, template) in hits {
            ctx.diagnostic(
                Diagnostic::warn(template).with_location(files::located(path, Some(line))),
            );
        }
    }
}

fn note_or_text(files: &crate::lint::file_tree::FileTree) -> HashSet<String> {
    let Some(loaded) = templates::load_templates(files) else {
        return HashSet::new();
    };
    loaded
        .templates
        .iter()
        .filter_map(|template| {
            let name = templates::template_name(template);
            let kind = template.value("Type");
            if !name.is_empty() && (kind == "note" || kind == "text") {
                Some(name)
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn with_note(script: &str, body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file(
                "templates",
                TestFile::new(
                    "\
Template: debconf/note-abuse
Type: note
Description: This is a note

Template: debconf/test
Type: text
Description: Useless note

Template: debconf/error
Type: error
Description: Not a note
",
                ),
            )
            .control_file(script, TestFile::new(body))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            with_note("config", "#!/bin/sh\ndb_input high debconf/note-abuse\n"),
            with_note(
                "config",
                "#!/bin/sh\nmakedev /dev/sda\ndb_input low debconf/note-abuse\n",
            ),
            with_note("config", "#!/bin/sh\ndb_input $foo debconf/test\n"),
            with_note(
                "config",
                "#!/bin/sh\ndb_input \"$foo\" debconf/note-abuse\n",
            ),
            with_note("config", "#!/bin/sh\ndb_input low debconf/error\n"),
            with_note("config", "db_input low debconf/note-abuse\n"),
            with_note("postinst", "#!/bin/sh\ndb_input low debconf/note-abuse\n").udeb(),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).control_file(
                "postinst",
                TestFile::new("#!/bin/sh\ndb_input low debconf/test\n"),
            ),
        ];
        let fail = vec![
            with_note("config", "#!/bin/sh\ndb_input low debconf/note-abuse\n"),
            with_note("config", "#!/bin/sh\ndb_input 'medium' debconf/test\n"),
            with_note("config", "#!/bin/sh\ndb_text low debconf/note-abuse\n"),
            with_note(
                "config",
                "#!/bin/sh\necho /dev/sda\n\ndb_input low debconf/note-abuse\n",
            ),
            with_note(
                "config",
                "#!/bin/sh\necho /dev/sda\ndb_input medium debconf/test\n",
            ),
            with_note("config", "#!/bin/sh\ndb_input \\\nlow debconf/note-abuse\n"),
            with_note("config", "#!/bin/sh\ndb_input low debconf/test\n").control_file(
                "postinst",
                TestFile::new("#!/bin/sh\ndb_input medium debconf/note-abuse\n"),
            ),
        ];
        BinaryPackageTester::new(PossibleDebconfNoteAbuse, pass, fail).test_and_snapshot();
    }
}
