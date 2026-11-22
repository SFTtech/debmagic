use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::{files, scripts, templates};

pub struct UnusedDebconfTemplate;

declare_rule! {
    /// A template in `templates` is not used by any maintainer script.
    ///
    /// Reads `templates` and the shebang maintainer scripts (`config`, `prerm`,
    /// `postrm`, `preinst`, `postinst`) as the Check does. A template is used
    /// when a `db_input`, `db_text`, `db_get`, `db_set` or `db_settitle` line
    /// names it (`$DPKG_MAINTSCRIPT_PACKAGE` stands for the package), or a
    /// Perl `get`, `set`, `metaget` or `settitle` call does, or a `db_register`
    /// alias of it does. Templates named `shared/packages-wordlist`,
    /// `shared/packages-ispell`, or ending in `/languages` are exempt, as is
    /// every template of `debconf` and of udebs. Extra is the template name;
    /// the Location is its `Template` field. Info, not selected by default.
    UnusedDebconfTemplate,
    code = "LN1323",
    tag = "unused-debconf-template",
    default_selected = false,
    experimental = false,
}

const MAINTSCRIPT_PACKAGE: &str = "$DPKG_MAINTSCRIPT_PACKAGE";

static DB_GET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?s)\A\s*(?:db_get|db_set(?:title)?)\s+["']?(\S+?)["']?(?:\s|\z)"#)
        .expect("valid regex")
});
static PERL_GET_SET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^\s*(?:.*=\s*get|set)\s*\(\s*["'](\S+?)["']"#).expect("valid regex")
});
static PERL_META: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\b(?:metaget|settitle)\s*\(\s*["'](\S+?)["']"#).expect("valid regex")
});
static DB_REGISTER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^\s*db_register\s+["']?(\S+?)["']?\s+(\S+)\s"#).expect("valid regex")
});
static EXEMPT_NAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^shared/packages-(wordlist|ispell)$|/languages$").expect("valid regex")
});

impl BinaryPackageRule for UnusedDebconfTemplate {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() || ctx.package_name() == "debconf" {
            return;
        }
        let package = ctx.package_name().to_string();
        let reports = {
            let Some(files) = ctx.control_files() else {
                return;
            };
            if !files::gate(files).open() {
                return;
            }
            let Some(loaded) = templates::load_templates(files) else {
                return;
            };
            let used = used_templates(files, &package);
            let mut reports = Vec::new();
            // Lintian's hash keeps the last template of a repeated name.
            let mut by_name: BTreeMap<String, &templates::Template> = BTreeMap::new();
            for template in &loaded.templates {
                if template.declares("Template") {
                    by_name.insert(template.value("Template"), template);
                }
            }
            for (name, template) in &by_name {
                let name = name.trim_end();
                if EXEMPT_NAME.is_match(name) || used.is_used(name) {
                    continue;
                }
                reports.push((
                    name.to_string(),
                    loaded.path.clone(),
                    template.position("Template"),
                ));
            }
            reports
        };
        for (name, path, line) in reports {
            ctx.diagnostic(
                Diagnostic::warn(name)
                    .with_severity(Severity::Info)
                    .with_location(files::located(path, line)),
            );
        }
    }
}

/// Template names the maintainer scripts use, and the `db_register` aliases
/// of each name.
#[derive(Default)]
struct Used {
    names: HashSet<String>,
    aliases: HashMap<String, Vec<String>>,
}

impl Used {
    fn is_used(&self, name: &str) -> bool {
        self.names.contains(name)
            || self
                .aliases
                .get(name)
                .is_some_and(|aliases| aliases.iter().any(|alias| self.names.contains(alias)))
    }
}

fn used_templates(files: &crate::lint::file_tree::FileTree, package: &str) -> Used {
    let mut used = Used::default();
    for script in scripts::scan(files) {
        for line in &script.lines {
            let text = line.text.as_str();
            if let Some(call) = scripts::db_invocation(text) {
                used.names.insert(maintscript_name(&call.template, package));
            }
            if let Some(captures) = DB_GET.captures(text) {
                used.names.insert(maintscript_name(&captures[1], package));
            }
            // Perl's `get`, `set`, `metaget` and `settitle` take the name as written.
            if let Some(captures) = PERL_GET_SET
                .captures(text)
                .or_else(|| PERL_META.captures(text))
            {
                used.names.insert(captures[1].to_string());
            }
            if let Some(captures) = DB_REGISTER.captures(text) {
                used.aliases
                    .entry(captures[1].to_string())
                    .or_default()
                    .push(captures[2].to_string());
            }
        }
    }
    used
}

/// lintian's `get_template_name`: a leading `$DPKG_MAINTSCRIPT_PACKAGE` becomes
/// the package name.
fn maintscript_name(name: &str, package: &str) -> String {
    match name.strip_prefix(MAINTSCRIPT_PACKAGE) {
        Some(rest) => format!("{package}{rest}"),
        None => name.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    const TEMPLATES: &str = "Template: example/used\nType: string\nDescription: Used\n Text.\n\n\
        Template: example/unused\nType: string\nDescription: Unused\n Text.\n";

    fn with_script(script: &str, body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file("templates", TestFile::new(TEMPLATES))
            .control_file("postinst", TestFile::new(body))
            .control_file("config", TestFile::new(script))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            with_script(
                "",
                "#!/bin/sh\n. /usr/share/debconf/confmodule\ndb_get example/used\ndb_get example/unused\n",
            ),
            with_script(
                "",
                "#!/bin/sh\ndb_input high example/used || true\ndb_get example/unused\n",
            ),
            with_script(
                "",
                "#!/bin/sh\ndb_register example/unused example/used\ndb_get example/used\n",
            ),
            with_script(
                "",
                "#!/bin/sh\ndb_get $DPKG_MAINTSCRIPT_PACKAGE/used\ndb_get $DPKG_MAINTSCRIPT_PACKAGE/unused\n",
            ),
            with_script(
                "",
                "#!/usr/bin/perl\nmy $x = get('example/used');\nmy $y = get('example/unused');\n",
            ),
        ];
        let fail = vec![
            with_script("", "#!/bin/sh\ndb_get example/used\n"),
            with_script("", "db_get example/used\n"),
        ];
        BinaryPackageTester::new(UnusedDebconfTemplate, pass, fail).test_and_snapshot();
    }
}
