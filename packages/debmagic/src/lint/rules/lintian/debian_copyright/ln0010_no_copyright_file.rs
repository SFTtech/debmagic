use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::rule::BinaryPackageRule;

pub struct NoCopyrightFile;

declare_rule! {
    /// The Binary package ships no copyright file.
    ///
    /// Looks at `usr/share/doc/<pkg>`. A symlink there is judged by other
    /// Tags and is not reported. Otherwise the package needs the directory
    /// and a `copyright` or `copyright.gz` entry in it (a symlink counts as
    /// present); the `perl` package is exempt from the file part. Udebs are
    /// not checked. Extra and Location are empty.
    NoCopyrightFile,
    code = "LN0010",
    tag = "no-copyright-file",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for NoCopyrightFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let package = ctx.package_name().to_string();
        let Some(files) = ctx.data_files() else {
            return;
        };
        if lacks_copyright_file(files, &package) {
            ctx.diagnostic(Diagnostic::error(""));
        }
    }
}

fn lacks_copyright_file(files: &FileTree, package: &str) -> bool {
    let doc_dir = format!("usr/share/doc/{package}");
    if files
        .get(&doc_dir)
        .is_some_and(|entry| entry.link_target().is_some())
    {
        return false;
    }
    let prefix = format!("{doc_dir}/");
    let has_directory = files.entries().iter().any(|entry| {
        entry
            .path()
            .to_str()
            .is_some_and(|path| path.starts_with(&prefix))
    }) || files
        .get(&doc_dir)
        .is_some_and(|entry| entry.kind() == EntryKind::Directory);
    if !has_directory {
        return true;
    }
    let found = ["copyright", "copyright.gz"]
        .iter()
        .any(|name| files.get(format!("{prefix}{name}")).is_some());
    !found && package != "perl"
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn files(paths: &[&str]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            paths
                .iter()
                .map(|path| (path.to_string(), TestFile::new("content")))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            files(&["usr/share/doc/example/copyright"]),
            files(&["usr/share/doc/example/copyright.gz"]),
            files(&["usr/share/doc/example/copyright", "usr/bin/example"]),
            // A symlinked copyright counts as present.
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/copyright",
                TestFile::symlink("../other/copyright"),
            )]),
            // A symlinked doc directory is judged elsewhere.
            BinaryPackageCase::from(vec![("usr/share/doc/example", TestFile::symlink("other"))]),
            // perl is exempt when its doc directory exists.
            files(&["usr/share/doc/perl/NEWS"]).package("perl"),
            // Udebs carry no copyright.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).udeb(),
        ];
        let fail = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            files(&["usr/bin/example"]),
            files(&["usr/share/doc/example/changelog.gz"]),
            // The directory entry alone is not a copyright file.
            BinaryPackageCase::from(vec![("usr/share/doc/example", TestFile::directory())]),
            // Another package's copyright does not count.
            files(&["usr/share/doc/other/copyright"]),
            // A file called `example` is not the doc directory.
            files(&["usr/share/doc/example"]),
            // A copyright deeper down is not the package's copyright file.
            files(&["usr/share/doc/example/sub/copyright"]),
            // perl without a doc directory is still reported.
            files(&["usr/bin/perl"]).package("perl"),
        ];
        BinaryPackageTester::new(NoCopyrightFile, pass, fail).test_and_snapshot();
    }
}
