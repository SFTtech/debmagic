use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::deb_members::Tarball;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct UnpackMessageForDebData;

declare_rule! {
    /// Unpacking the `data.tar` member of an installable package printed an
    /// error. It probably means something is broken, or the package was built
    /// in a strange way.
    ///
    /// Reports each message tar prints while the Data tarball is unpacked. The
    /// message is GNU tar's: `tar: Removing leading `/' from member names`
    /// when a member name starts with a slash (once per archive). The extra is
    /// the message. Deb and udeb alike. The Control tarball is
    /// `UnpackMessageForDebControl`.
    UnpackMessageForDebData,
    code = "LN1433",
    tag = "unpack-message-for-deb-data",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for UnpackMessageForDebData {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let messages = ctx.unpack_messages();
        for message in messages.get(Tarball::Data) {
            ctx.diagnostic(Diagnostic::error(message.clone()));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::binary_package_reader::write_test_deb_with_raw_names;
    use crate::lint::context::{BinaryPackageContext, BinaryPackageData};

    use super::*;

    const CONTROL: &[u8] = b"Package: pkg\nVersion: 1.0\nArchitecture: all\nMaintainer: a <a@example.org>\nDescription: test\n test\n";

    /// Messages of one lint of a `.deb` (or `.udeb`) with these members.
    fn messages(file_name: &str, data: &[(&str, &[u8])]) -> Vec<String> {
        let dir = tempfile::TempDir::new().expect("temp directory");
        let path = dir.path().join(file_name);
        write_test_deb_with_raw_names(&path, &[("control", CONTROL)], data).expect("deb");
        let data = BinaryPackageData::new(path).expect("Binary package");
        let mut ctx = BinaryPackageContext::bind(&data, &UnpackMessageForDebData);
        UnpackMessageForDebData.run(&mut ctx);
        ctx.finish()
            .expect("no runtime error")
            .into_iter()
            .map(|diagnostic| diagnostic.message.clone())
            .collect()
    }

    #[test]
    fn leading_slash_members_are_reported_once() {
        let found = messages(
            "pkg_1.0_all.deb",
            &[("/usr/bin/a", b"a"), ("/usr/bin/b", b"b")],
        );
        assert_eq!(found, ["tar: Removing leading `/' from member names"]);
    }

    #[test]
    fn udeb_is_checked_too() {
        let found = messages("pkg_1.0_all.udeb", &[("/usr/bin/a", b"a")]);
        assert_eq!(found, ["tar: Removing leading `/' from member names"]);
    }

    #[test]
    fn relative_member_names_are_not_reported() {
        let found = messages(
            "pkg_1.0_all.deb",
            &[("./usr/bin/a", b"a"), ("usr/bin/b", b"b")],
        );
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn a_leading_slash_in_control_only_is_not_this_tag() {
        let dir = tempfile::TempDir::new().expect("temp directory");
        let path = dir.path().join("pkg_1.0_all.deb");
        write_test_deb_with_raw_names(&path, &[("/control", CONTROL)], &[("usr/bin/a", b"a")])
            .expect("deb");
        let data = BinaryPackageData::new(path).expect("Binary package");
        let mut ctx = BinaryPackageContext::bind(&data, &UnpackMessageForDebData);
        UnpackMessageForDebData.run(&mut ctx);
        assert!(ctx.finish().expect("no runtime error").is_empty());
    }
}
