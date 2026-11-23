use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::deb_members::Tarball;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct UnpackMessageForDebControl;

declare_rule! {
    /// Unpacking the `control.tar` member of an installable package printed an
    /// error. It probably means something is broken, or the package was built
    /// in a strange way.
    ///
    /// Reports each message tar prints while the Control tarball is unpacked,
    /// as `UnpackMessageForDebData` does for the Data tarball. The message is
    /// GNU tar's: `tar: Removing leading `/' from member names` when a member
    /// name starts with a slash (once per archive). The extra is the message.
    /// Deb and udeb alike.
    UnpackMessageForDebControl,
    code = "LN1591",
    tag = "unpack-message-for-deb-control",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for UnpackMessageForDebControl {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let messages = ctx.unpack_messages();
        for message in messages.get(Tarball::Control) {
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
    fn messages(file_name: &str, control: &[(&str, &[u8])], data: &[(&str, &[u8])]) -> Vec<String> {
        let dir = tempfile::TempDir::new().expect("temp directory");
        let path = dir.path().join(file_name);
        write_test_deb_with_raw_names(&path, control, data).expect("deb");
        let data = BinaryPackageData::new(path).expect("Binary package");
        let mut ctx = BinaryPackageContext::bind(&data, &UnpackMessageForDebControl);
        UnpackMessageForDebControl.run(&mut ctx);
        ctx.finish()
            .expect("no runtime error")
            .into_iter()
            .map(|diagnostic| diagnostic.message.clone())
            .collect()
    }

    #[test]
    fn leading_slash_control_members_are_reported_once() {
        let found = messages(
            "pkg_1.0_all.deb",
            &[("/control", CONTROL), ("/md5sums", b"")],
            &[("usr/bin/a", b"a")],
        );
        assert_eq!(found, ["tar: Removing leading `/' from member names"]);
    }

    #[test]
    fn udeb_is_checked_too() {
        let found = messages(
            "pkg_1.0_all.udeb",
            &[("/control", CONTROL)],
            &[("usr/bin/a", b"a")],
        );
        assert_eq!(found, ["tar: Removing leading `/' from member names"]);
    }

    #[test]
    fn relative_control_member_names_are_not_reported() {
        let found = messages(
            "pkg_1.0_all.deb",
            &[("./control", CONTROL), ("md5sums", b"")],
            &[("usr/bin/a", b"a")],
        );
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn a_leading_slash_in_data_only_is_not_this_tag() {
        let found = messages(
            "pkg_1.0_all.deb",
            &[("control", CONTROL)],
            &[("/usr/bin/a", b"a")],
        );
        assert!(found.is_empty(), "{found:?}");
    }
}
