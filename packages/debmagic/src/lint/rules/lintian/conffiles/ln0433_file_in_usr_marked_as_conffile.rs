use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct FileInUsrMarkedAsConffile;

declare_rule! {
    /// A `conffiles` entry's path starts with `usr/`.
    ///
    /// The control member is parsed as lintian does: right-trimmed lines, one
    /// path per line (the last word), a single leading `/` removed. Each
    /// `usr/` entry is reported, including duplicates. The file does not have
    /// to be installed. Extra is the path without that leading slash. This is
    /// the `.deb` check, so udebs are not checked.
    FileInUsrMarkedAsConffile,
    code = "LN0433",
    tag = "file-in-usr-marked-as-conffile",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for FileInUsrMarkedAsConffile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(text) = control_text(ctx, "conffiles") else {
            return;
        };
        for path in usr_paths(&text) {
            ctx.diagnostic(Diagnostic::error(path));
        }
    }
}

fn control_text(ctx: &mut BinaryPackageContext<'_>, name: &str) -> Option<String> {
    let files = ctx.control_files()?;
    let entry = files.get(name)?;
    let bytes = files.read(entry).ok()?;
    String::from_utf8(bytes).ok()
}

fn usr_paths(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim_end();
            if line.is_empty() {
                return None;
            }
            let path = line.split_whitespace().next_back()?;
            let path = path.strip_prefix('/').unwrap_or(path);
            path.starts_with("usr/").then(|| path.to_string())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn conffiles(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file("conffiles", TestFile::new(body))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            conffiles("/etc/foo\n/var/lib/foo\n"),
            conffiles("//usr/bin/foo\n"),
            conffiles("/usr/bin/foo\n").udeb(),
        ];
        let fail = vec![
            conffiles("/usr/share/foo\n/etc/foo\n/usr/gone\n"),
            conffiles("remove-on-upgrade /usr/bin/foo\n"),
            conffiles("/usr/bin/foo\n/usr/bin/foo\n"),
            conffiles("usr/lib/foo\n"),
        ];
        BinaryPackageTester::new(FileInUsrMarkedAsConffile, pass, fail).test_and_snapshot();
    }
}
