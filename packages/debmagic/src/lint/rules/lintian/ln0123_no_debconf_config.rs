use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct NoDebconfConfig;

declare_rule! {
    /// The package ships debconf `templates` and no `config` script.
    ///
    /// A `preinst` that loads `/usr/share/debconf/confmodule` or
    /// `Debconf::Client::ConfModule` counts as config. Udebs are not checked.
    /// Extra and Location are empty.
    NoDebconfConfig,
    code = "LN0123",
    tag = "no-debconf-config",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for NoDebconfConfig {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(files) = ctx.control_files() else {
            return;
        };
        let templates = files.get("templates").is_some_and(|entry| {
            matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink)
        });
        let config = files.get("config").is_some_and(|entry| {
            matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink)
        });
        if !templates || config || preinst_uses_debconf(ctx) {
            return;
        }
        ctx.diagnostic(Diagnostic::error(""));
    }
}

fn preinst_uses_debconf(ctx: &mut BinaryPackageContext<'_>) -> bool {
    let Some(files) = ctx.control_files() else {
        return false;
    };
    let Some(entry) = files.get("preinst") else {
        return false;
    };
    if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
        return false;
    }
    let Ok(bytes) = files.read(entry) else {
        return false;
    };
    let Ok(text) = String::from_utf8(bytes) else {
        return false;
    };
    text.lines().any(|line| {
        let code = line.split('#').next().unwrap_or("");
        code.contains("/usr/share/debconf/confmodule")
            || code.contains("Debconf::Client::ConfModule")
            || code.contains("Debian::DebConf::Client::ConfModule")
    })
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn templates() -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).control_file(
            "templates",
            TestFile::new("Template: example/foo\nType: string\n"),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            templates().control_file("config", TestFile::new("#!/bin/sh\n")),
            templates().control_file(
                "preinst",
                TestFile::new("#!/bin/sh\n. /usr/share/debconf/confmodule\n"),
            ),
            templates().udeb(),
        ];
        let fail = vec![templates()];
        BinaryPackageTester::new(NoDebconfConfig, pass, fail).test_and_snapshot();
    }
}
