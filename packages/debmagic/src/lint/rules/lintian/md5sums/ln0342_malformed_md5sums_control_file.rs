use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::text;

pub struct MalformedMd5sumsControlFile;

declare_rule! {
    /// A non-empty line of the md5sums control file is not a 32-character
    /// checksum, two spaces, and a path.
    ///
    /// Extra is `Odd text: ` plus the line, or `Empty name for checksum `
    /// plus the checksum. The Location is `md5sums` with no line. An empty
    /// file and a symlink are skipped.
    MalformedMd5sumsControlFile,
    code = "LN0342",
    tag = "malformed-md5sums-control-file",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MalformedMd5sumsControlFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(file) = text::load(ctx) else {
            return;
        };
        for error in file.errors {
            ctx.diagnostic(Diagnostic::error(error).with_location(text::at()));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn md5sums(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file("md5sums", TestFile::new(body))
    }

    #[test]
    fn test() {
        let checksum = "5d41402abc4b2a76b9719d911017c592";
        let pass = vec![
            md5sums(&format!("{checksum}  usr/bin/foo\n")),
            md5sums(""),
            md5sums("\n\n"),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("md5sums", TestFile::symlink("other")),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("md5sums", TestFile::directory()),
        ];
        let fail = vec![
            md5sums("broken-md5sums\n"),
            md5sums("this is a malformed line\n"),
            md5sums(&format!("{checksum} usr/bin/foo\n")),
            md5sums("not-32-chars  usr/bin/foo\n").udeb(),
        ];
        BinaryPackageTester::new(MalformedMd5sumsControlFile, pass, fail).test_and_snapshot();
    }
}
