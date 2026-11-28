//! lintian's `debian/po-debconf` Check, from `$has_template` down to the
//! `intltool-update --pot` comparison and the translation statistics. Shared
//! by LN1326 (invalid-potfiles-in), LN1327 (newer-debconf-templates) and LN1531
//! (no-complete-debconf-translation), which read it through [`state`]. It
//! runs once per Subject.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use regex::Regex;

use crate::lint::context::SourceContext;
use crate::lint::external_tool::{self, Sandbox};
use crate::lint::file_tree::{Entry, EntryKind, FileTree};

use super::files;
use super::msgfmt;

/// intltool's wrapper, which lintian calls by this absolute path.
const INTLTOOL_UPDATE: &str = "/usr/share/intltool-debian/intltool-update";
const INTLTOOL_EXTRACT: &str = "/usr/share/intltool-debian/intltool-extract";

/// `msgfmt --statistics` on a complete translation: `N translated messages.`
static FULL_STATISTICS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\w+ \w+ \w+\.$").expect("valid regex"));

/// What the `intltool-update --pot` comparison found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Pot {
    /// Not run: no PO directory, or POTFILES.in, a file it lists, or
    /// templates.pot is missing.
    NotRun,
    /// `intltool-update --pot` failed (LN1326).
    Invalid,
    /// The generated template matches templates.pot both ways.
    Current,
    /// The generated template differs from templates.pot (LN1327).
    Stale,
    /// The tools could not run, so the Check has no verdict.
    Unknown,
}

/// The po-debconf facts the Rules share, for one Subject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PoState {
    /// `$has_template` and a PO directory: the package uses po-debconf.
    pub(super) using_po: bool,
    /// `debian/po/POTFILES.in` (or `po.in`), when `using_po`.
    pub(super) potfiles_in: Option<PathBuf>,
    /// `templates.pot` in the PO directory, when `using_po`.
    pub(super) templates_pot: Option<PathBuf>,
    pub(super) pot: Pot,
    /// Whether some translation is complete (`msgfmt` reports only
    /// translated messages). `None` when the tool could not run.
    pub(super) full_translation: Option<bool>,
}

impl PoState {
    fn not_using() -> Self {
        Self {
            using_po: false,
            potfiles_in: None,
            templates_pot: None,
            pot: Pot::NotRun,
            full_translation: None,
        }
    }
}

/// The shared state of `ctx`'s Subject, computed on first use.
pub(super) fn state(ctx: &mut SourceContext<'_>) -> Arc<PoState> {
    ctx.shared_once(|ctx| match ctx.files() {
        Some(tree) => analyse(ctx, tree),
        None => PoState::not_using(),
    })
}

/// The Subject's `debian/` files staged for host tools, built once and shared
/// by the PO Rules. `None` when staging fails.
pub(super) fn staged(ctx: &mut SourceContext<'_>) -> Arc<StagedDebian> {
    ctx.shared_once(|ctx| {
        StagedDebian(match ctx.files() {
            Some(tree) => {
                let entries: Vec<&Entry> = tree
                    .entries()
                    .iter()
                    .filter(|entry| entry.path().starts_with("debian"))
                    .collect();
                Sandbox::new(tree, &entries).ok()
            }
            None => None,
        })
    })
}

/// The result of [`staged`]. A type of its own because `shared_once` keys its
/// slots by type, and a bare `Option<Sandbox>` could collide with another user.
pub(super) struct StagedDebian(pub(super) Option<Sandbox>);

fn analyse(ctx: &mut SourceContext<'_>, tree: &FileTree) -> PoState {
    if !files::has_translatable_template(tree) {
        return PoState::not_using();
    }
    let Some(directory) = files::po_directory(tree) else {
        return PoState::not_using();
    };
    let po = Path::new(directory);
    let potfiles_in = po.join("POTFILES.in");
    let templates_pot = po.join("templates.pot");
    let mut state = PoState {
        using_po: true,
        potfiles_in: Some(potfiles_in.clone()),
        templates_pot: Some(templates_pot.clone()),
        pot: Pot::NotRun,
        full_translation: None,
    };

    let staged = staged(ctx);
    let Some(sandbox) = &staged.0 else {
        state.pot = Pot::Unknown;
        return state;
    };

    if inputs_complete(tree, &potfiles_in, &templates_pot) {
        state.pot = compare_pot(sandbox, po);
    }
    if state.pot != Pot::Invalid {
        let statistics = msgfmt::statistics(ctx);
        state.full_translation = full_translation(tree, &statistics, directory);
    }
    state
}

/// lintian's `missing_files == 0`: POTFILES.in exists, every file it lists
/// under `debian/` is a file, and templates.pot exists.
fn inputs_complete(tree: &FileTree, potfiles_in: &Path, templates_pot: &Path) -> bool {
    let Some(entry) = files::open_file(tree, potfiles_in) else {
        return false;
    };
    let Ok(bytes) = tree.read(entry) else {
        return false;
    };
    let mut complete = true;
    for raw in bytes.split(|&byte| byte == b'\n') {
        let line = String::from_utf8_lossy(raw);
        if line.trim_start().starts_with('#') {
            continue;
        }
        // lintian's `s/.*\]\s*//`: drop everything up to the last `]`.
        let rest = match line.rfind(']') {
            Some(index) => line[index + 1..].trim_start(),
            None => &line,
        };
        if rest.is_empty() {
            continue;
        }
        let listed = Path::new("debian").join(rest);
        let is_file = files::resolve(tree, &listed).is_some_and(|entry| {
            matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink)
        });
        if !is_file {
            complete = false;
        }
    }
    complete && files::open_file(tree, templates_pot).is_some()
}

/// lintian's `intltool-update --pot` in the PO directory, then `msgcmp
/// --use-untranslated` both ways against templates.pot.
fn compare_pot(sandbox: &Sandbox, po: &Path) -> Pot {
    let (Some(intltool), Some(msgcmp)) = (
        external_tool::find(INTLTOOL_UPDATE),
        external_tool::find("msgcmp"),
    ) else {
        return Pot::Unknown;
    };
    let srcdir = sandbox.root().join(po);
    let generated = po.join("test.pot");
    let templates = po.join("templates.pot");
    let update = sandbox.run(
        &intltool,
        &[OsStr::new("--gettext-package=test"), OsStr::new("--pot")],
        po,
        &[
            ("INTLTOOL_EXTRACT", OsStr::new(INTLTOOL_EXTRACT)),
            ("srcdir", srcdir.as_os_str()),
        ],
        external_tool::DEFAULT_TIMEOUT,
    );
    match update {
        Err(_) => return Pot::Unknown,
        Ok(captured) if !captured.success() => return Pot::Invalid,
        Ok(_) => {}
    }
    let left = sandbox.run(
        &msgcmp,
        &[
            OsStr::new("--use-untranslated"),
            generated.as_os_str(),
            templates.as_os_str(),
        ],
        Path::new(""),
        &[],
        external_tool::DEFAULT_TIMEOUT,
    );
    let right = sandbox.run(
        &msgcmp,
        &[
            OsStr::new("--use-untranslated"),
            templates.as_os_str(),
            generated.as_os_str(),
        ],
        Path::new(""),
        &[],
        external_tool::DEFAULT_TIMEOUT,
    );
    match (left, right) {
        (Ok(left), Ok(right)) if left.success() && right.success() => Pot::Current,
        (Ok(_), Ok(_)) => Pot::Stale,
        _ => Pot::Unknown,
    }
}

/// lintian's loop over the PO directory: a `.po` file with a `msgstr` line
/// that `msgfmt --statistics` reports as fully translated. `None` when the
/// tool could not run.
fn full_translation(
    tree: &FileTree,
    statistics: &msgfmt::Statistics,
    directory: &str,
) -> Option<bool> {
    if !statistics.found {
        return None;
    }
    let mut full = false;
    for entry in files::po_children(tree, directory) {
        if !msgfmt::is_checked(tree, entry) {
            continue;
        }
        let run = statistics.run(entry.path())?;
        // Perl's `$stats` is stderr; its `$` also matches before one final newline.
        let stderr = String::from_utf8_lossy(&run.stderr);
        let stderr = stderr.strip_suffix('\n').unwrap_or(&stderr);
        if FULL_STATISTICS.is_match(stderr) {
            full = true;
        }
    }
    Some(full)
}

#[cfg(test)]
pub(super) mod test_support {
    use crate::lint::tester::TestFile;

    pub(in crate::lint::rules::lintian) const CONTROL: &str =
        "Source: example\n\nPackage: example\nArchitecture: all\nDescription: example\n extra\n";
    pub(in crate::lint::rules::lintian) const TEMPLATES: &str =
        "Template: example/foo\nType: string\n_Description: Ask\n Question.\n";
    pub(in crate::lint::rules::lintian) const POTFILES_IN: &str =
        "[type: gettext/rfc822deb] templates\n";
    /// What `intltool-update --pot` generates from [`TEMPLATES`], without the
    /// header lines (msgcmp compares the messages).
    pub(in crate::lint::rules::lintian) const TEMPLATES_POT: &str = "msgid \"\"\nmsgstr \"\"\n\n#. Description\n#: ../templates:1001\nmsgid \"Ask\"\nmsgstr \"\"\n\n#. Description\n#: ../templates:1001\nmsgid \"Question.\"\nmsgstr \"\"\n";

    /// A Source tree with a debconf template, POTFILES.in, and templates.pot,
    /// with `extra` replacing or adding files.
    pub(in crate::lint::rules::lintian) fn tree(
        extra: Vec<(&str, TestFile)>,
    ) -> Vec<(String, TestFile)> {
        let mut files: Vec<(String, TestFile)> = vec![
            ("debian/control".to_string(), TestFile::from(CONTROL)),
            ("debian/po".to_string(), TestFile::directory()),
            ("debian/templates".to_string(), TestFile::from(TEMPLATES)),
            (
                "debian/po/POTFILES.in".to_string(),
                TestFile::from(POTFILES_IN),
            ),
            (
                "debian/po/templates.pot".to_string(),
                TestFile::from(TEMPLATES_POT),
            ),
        ];
        files.retain(|(path, _)| !extra.iter().any(|(name, _)| *name == path));
        files.extend(
            extra
                .into_iter()
                .map(|(path, file)| (path.to_string(), file)),
        );
        files
    }
}
