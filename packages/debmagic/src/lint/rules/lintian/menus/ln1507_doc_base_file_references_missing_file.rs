use crate::declare_rule;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::group_context::GroupContext;
use crate::lint::rule::GroupRule;

use super::doc_base::{ReferenceTargets, missing_references};

pub struct DocBaseFileReferencesMissingFile;

declare_rule! {
    /// An `Index` or `Files` entry of a doc-base file names a file that
    /// neither the package nor an installable it depends on ships. Extra is
    /// the entry; the Location is the field's last line.
    ///
    /// The doc-base files are the direct children of `usr/share/doc-base`
    /// that are UTF-8 and not executable. The files looked in are those of
    /// the package and of every installable of the Group that its
    /// Pre-Depends or Depends satisfy, with symlinks on the way to the entry
    /// followed (also those of the dependencies). An entry may be a directory.
    /// A wildcard in `Files` matches within one path component, and an entry
    /// with a wildcard and a character class (`[`) is not checked; in `Index` a wildcard is
    /// part of the name.
    ///
    /// A Group Rule: it needs the files of the dependencies, so it runs once
    /// per Group that has a `.dsc`, and the Diagnostic attaches to the
    /// installable with the doc-base file (binary or udeb), as in lintian.
    DocBaseFileReferencesMissingFile,
    code = "LN1507",
    tag = "doc-base-file-references-missing-file",
    default_selected = true,
    experimental = false,
}

impl GroupRule for DocBaseFileReferencesMissingFile {
    fn run(&self, ctx: &mut GroupContext<'_>) {
        let installables = ctx.installables();
        for installable in &installables {
            let Some(files) = ctx.data_files(installable) else {
                continue;
            };
            // Most packages have no doc-base file: build the targets last.
            if !files
                .entries()
                .iter()
                .any(|entry| entry.path().starts_with("usr/share/doc-base"))
            {
                continue;
            }
            let strong = installable.strong_relation();
            let mut targets = ReferenceTargets::default();
            for other in &installables {
                if other.name() != installable.name() && !strong.satisfies(other.name()) {
                    continue;
                }
                if let Some(tree) = ctx.data_files(other) {
                    targets.add_tree(tree);
                }
            }
            for (path, line, entry) in missing_references(files, &targets) {
                ctx.installable_diagnostic(
                    installable,
                    Diagnostic::error(entry).with_location(Location {
                        path,
                        line: Some(line),
                        column: None,
                    }),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{GroupCase, GroupTester, TestFile};

    use super::*;

    const DSC: &str = "Source: example\nVersion: 1.0\n";

    fn doc(body: &str) -> Vec<(&'static str, TestFile)> {
        vec![("usr/share/doc-base/example", TestFile::new(body))]
    }

    fn with(body: &str, extra: Vec<(&'static str, TestFile)>) -> Vec<(&'static str, TestFile)> {
        let mut files = doc(body);
        files.extend(extra);
        files
    }

    fn file(path: &'static str) -> (&'static str, TestFile) {
        (path, TestFile::new(""))
    }

    fn files_entry(entries: &str) -> String {
        format!("Document: example\nTitle: T\nSection: Help\n\nFormat: text\nFiles: {entries}\n")
    }

    #[test]
    fn group_rule() {
        let pass = vec![
            // Present in the package, in a dependency, as a directory.
            GroupCase::new(DSC)
                .installable("example", "")
                .files(with(&files_entry("/usr/share/doc/example/a.txt"), vec![file("usr/share/doc/example/a.txt")])),
            GroupCase::new(DSC)
                .installable("example", "Depends: dep\n")
                .files(doc(&files_entry("/usr/share/doc/dep/a.txt")))
                .installable("dep", "")
                .files([file("usr/share/doc/dep/a.txt")]),
            GroupCase::new(DSC)
                .installable("example", "")
                .files(with(
                    &files_entry("/usr/share/doc/example/dir"),
                    vec![("usr/share/doc/example/dir", TestFile::directory())],
                )),
            // Wildcards.
            GroupCase::new(DSC)
                .installable("example", "")
                .files(with(
                    &files_entry("/usr/share/doc/example/*.html /usr/share/doc/example/ch?.txt"),
                    vec![
                        file("usr/share/doc/example/index.html"),
                        file("usr/share/doc/example/ch1.txt"),
                    ],
                )),
            // A character class is not checked.
            GroupCase::new(DSC)
                .installable("example", "")
                .files(doc(&files_entry("/usr/share/doc/example/[ab]*.html"))),
            // Through a symlink, in the package or a dependency.
            GroupCase::new(DSC)
                .installable("example", "Pre-Depends: dep\n")
                .files(doc(&files_entry("/usr/share/doc/example/a.txt")))
                .installable("dep", "")
                .files([
                    ("usr/share/doc/example", TestFile::symlink("../real")),
                    file("usr/share/real/a.txt"),
                ]),
            GroupCase::new(DSC)
                .installable("example", "")
                .files(with(
                    &files_entry("/usr/share/doc/example/a.txt"),
                    vec![
                        ("usr/share/doc/example", TestFile::symlink("dir")),
                        file("usr/share/doc/dir/a.txt"),
                    ],
                )),
            // An Index.
            GroupCase::new(DSC)
                .installable("example", "")
                .files(with(
                    "Document: example\n\nFormat: html\nIndex: /usr/share/doc/example/index.html\nFiles: /usr/share/doc/example/*.html\n",
                    vec![file("usr/share/doc/example/index.html")],
                )),
            // An executable doc-base file is another Tag's business.
            GroupCase::new(DSC).installable("example", "").files([(
                "usr/share/doc-base/example",
                TestFile::new(files_entry("/missing")).mode(0o755),
            )]),
            // A doc-base file that is not UTF-8, in a subdirectory, or none.
            GroupCase::new(DSC).installable("example", "").files([(
                "usr/share/doc-base/example",
                TestFile::from_bytes(b"Files: /m\xe9\n".to_vec()),
            )]),
            GroupCase::new(DSC)
                .installable("example", "")
                .files([("usr/share/doc-base/sub/example", TestFile::new(files_entry("/missing")))]),
            GroupCase::new(DSC).installable("example", ""),
        ];
        let fail = vec![
            GroupCase::new(DSC)
                .installable("example", "")
                .files(doc(&files_entry("/usr/share/doc/example/a.txt"))),
            // A package that is not depended on, weakly or not at all.
            GroupCase::new(DSC)
                .installable("example", "Recommends: other\n")
                .files(doc(&files_entry("/usr/share/doc/other/a.txt")))
                .installable("other", "")
                .files([file("usr/share/doc/other/a.txt")]),
            // Several entries over continuation lines, a wildcard that
            // matches nothing across a directory, an Index with a wildcard.
            GroupCase::new(DSC)
                .installable("example", "")
                .files(with(
                    "Document: example\n\nFormat: html\nIndex: /usr/share/doc/example/in*.html\nFiles: /usr/share/doc/example/a.txt\n /usr/share/doc/example/*.html /missing\n",
                    vec![file("usr/share/doc/example/sub/x.html")],
                )),
            // A character class without a wildcard is a plain name.
            GroupCase::new(DSC)
                .installable("example", "")
                .files(doc(&files_entry("/usr/share/doc/example/[ab].html"))),
            // A udeb is looked at too.
            GroupCase::new(DSC)
                .udeb("example-udeb", "")
                .files(doc(&files_entry("/usr/share/doc/example/a.txt"))),
        ];
        GroupTester::new(DocBaseFileReferencesMissingFile, pass, fail).test_and_snapshot();
    }
}
