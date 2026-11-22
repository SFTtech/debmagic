use crate::declare_rule;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::group_context::ChangesContext;
use crate::lint::rule::ChangesRule;

pub struct BadSectionInChangesFile;

declare_rule! {
    /// Reports a file in the `Files` field of the `.changes` whose section is
    /// the bare `non-free` or `contrib`, which are no longer valid: the
    /// section has to name an archive area and a section, as in
    /// `non-free/xxx`. The extra is the file name and its section.
    ///
    /// Only the `Files` field has sections, so a file named only in a
    /// `Checksums-*` field is not looked at. Lintian reports the files in
    /// hash order; this Rule reports them in the order of the field.
    BadSectionInChangesFile,
    code = "LN1440",
    tag = "bad-section-in-changes-file",
    default_selected = true,
    experimental = false,
}

impl ChangesRule for BadSectionInChangesFile {
    fn run(&self, ctx: &mut ChangesContext<'_>) {
        let bad: Vec<String> = ctx
            .changes()
            .files()
            .files()
            .iter()
            .filter_map(|file| {
                let section = file
                    .section()
                    .filter(|s| matches!(*s, "non-free" | "contrib"))?;
                Some(format!("{} {section}", file.name()))
            })
            .collect();
        for extra in bad {
            ctx.diagnostic(Diagnostic::error(extra));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{ChangesCase, ChangesTester};

    use super::*;

    fn changes(files: &[(&str, &str)]) -> ChangesCase {
        let mut text = String::from("Source: example\nVersion: 1.0\nFiles:\n");
        for (section, name) in files {
            text.push_str(&format!(
                " 8de2b87704ef0a11144b57d243db88ec 12 {section} optional {name}\n"
            ));
        }
        ChangesCase::from(text)
    }

    #[test]
    fn changes_file() {
        let pass = vec![
            changes(&[]),
            changes(&[
                ("main/libs", "a_1.0_all.deb"),
                ("non-free/libs", "b_1.0_all.deb"),
            ]),
            changes(&[
                ("contrib/devel", "c_1.0_all.deb"),
                ("utils", "d_1.0_all.deb"),
            ]),
            // Only a Checksums field names it: no section to look at.
            ChangesCase::from(
                "Source: example\nChecksums-Sha1:\n da39a3ee5e6b4b0d3255bfef95601890afd80709 0 e_1.0_all.deb\n",
            ),
        ];
        let fail = vec![
            changes(&[("non-free", "a_1.0_all.deb")]),
            changes(&[("contrib", "a_1.0_all.deb")]),
            changes(&[
                ("main", "a_1.0_all.deb"),
                ("non-free", "b_1.0_all.deb"),
                ("contrib", "c_1.0_all.deb"),
            ]),
        ];
        ChangesTester::new(BadSectionInChangesFile, pass, fail).test_and_snapshot();
    }
}
