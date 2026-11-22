//! Shared menu-file and desktop-file parse for the menu-format Rules.
//!
//! Lintian reads direct children of `usr/share/menu` and `usr/lib/menu`, and
//! `.desktop` files directly under `usr/share/applications` and
//! `usr/share/xsessions`. Menu line numbers follow its counter: leading
//! comments and blank lines are not counted, and a continued item is reported
//! on its last physical line. Desktop line numbers count every physical line.
//! There is no file mode on an Entry, so executable menu files are still read.

use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};

use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::rule::Severity;
use regex::Regex;

use super::data;

/// Lintian's `@known_tags`: the menu item tags debmagic knows.
const KNOWN_MENU_TAGS: &[&str] = &[
    "needs",
    "section",
    "title",
    "sort",
    "command",
    "longtitle",
    "icon",
    "icon16x16",
    "icon32x32",
    "description",
    "hotkey",
    "hints",
];

pub(super) struct Finding {
    pub path: PathBuf,
    pub line: Option<u32>,
    pub message: String,
}

pub(super) struct Report {
    pub menu_command_not_in_package: Vec<Finding>,
    pub command_in_menu_file_and_desktop_file: Vec<Finding>,
    pub menu_item_creates_new_section: Vec<Finding>,
    pub desktop_command_not_in_package: Vec<Finding>,
    pub desktop_entry_contains_unknown_key: Vec<Finding>,
    pub menu_item_uses_apps_section: Vec<Finding>,
    pub menu_item_uses_windowmanagers_section: Vec<Finding>,
    pub non_wm_in_windowmanager_menu_section: Vec<Finding>,
    pub non_wm_module_in_wm_modules_menu_section: Vec<Finding>,
    pub menu_item_needs_tag_has_unknown_value: Vec<Finding>,
    pub desktop_entry_file_has_crs: Vec<Finding>,
    pub desktop_entry_invalid_category: Vec<Finding>,
    pub desktop_entry_uses_reserved_category: Vec<Finding>,
    pub pkg_not_in_package_test: Vec<Finding>,
    pub unquoted_string_in_menu_item: Vec<Finding>,
    pub su_wrapper_without_c: Vec<Finding>,
    pub su_wrapper_not_su_to_root: Vec<Finding>,
    pub menu_icon_uses_relative_path: Vec<Finding>,
    pub menu_icon_missing: Vec<Finding>,
    pub menu_item_missing_required_tag: Vec<Finding>,
    pub su_to_root_with_usr_sbin: Vec<Finding>,
    pub bad_test_in_menu_item: Vec<Finding>,
    pub duplicate_tag_in_menu: Vec<Finding>,
    pub desktop_contains_deprecated_key: Vec<Finding>,
    pub desktop_entry_contains_deprecated_key: Vec<Finding>,
    pub desktop_entry_contains_encoding_key: Vec<Finding>,
    pub desktop_entry_lacks_keywords_entry: Vec<Finding>,
    pub duplicate_key_in_desktop: Vec<Finding>,
    pub desktop_entry_unknown_type: Vec<Finding>,
    pub desktop_entry_missing_required_key: Vec<Finding>,
    pub desktop_entry_lacks_main_category: Vec<Finding>,
    pub desktop_mime_but_no_exec_code: Vec<Finding>,
    pub menu_item_contains_unknown_tag: Vec<Finding>,
    pub menu_item_creates_new_root_section: Vec<Finding>,
    pub menu_item_needs_dwww: Vec<Finding>,
    pub menu_item_uses_apps_games_section: Vec<Finding>,
    pub menu_item_uses_icon_none: Vec<Finding>,
    pub menu_icon_not_in_xpm_format: Vec<Finding>,
    pub menu_icon_too_big: Vec<Finding>,
    pub menu_icon_cannot_be_parsed: Vec<Finding>,
    pub unparsable_menu_item: Vec<Finding>,
    pub whitespace_after_continuation_character: Vec<Finding>,
}

pub(super) fn take(
    ctx: &mut BinaryPackageContext<'_>,
    pick: impl FnOnce(Report) -> Vec<Finding>,
) -> Vec<Finding> {
    let package = ctx.package_name().to_string();
    let Some(files) = ctx.data_files() else {
        return Vec::new();
    };
    pick(report(files, &package))
}

pub(super) fn warn(ctx: &mut BinaryPackageContext<'_>, finding: &Finding) {
    ctx.diagnostic(Diagnostic::warn(finding.message.clone()).with_location(location(finding)));
}

pub(super) fn error(ctx: &mut BinaryPackageContext<'_>, finding: &Finding) {
    ctx.diagnostic(Diagnostic::error(finding.message.clone()).with_location(location(finding)));
}

/// Reports `finding` as an info hint (`Diagnostic::warn` with Info severity).
pub(super) fn info(ctx: &mut BinaryPackageContext<'_>, finding: &Finding) {
    ctx.diagnostic(
        Diagnostic::warn(finding.message.clone())
            .with_severity(Severity::Info)
            .with_location(location(finding)),
    );
}

fn location(finding: &Finding) -> Location {
    Location {
        path: finding.path.clone(),
        line: finding.line,
        column: None,
    }
}

pub(super) fn report(files: &FileTree, package: &str) -> Report {
    let mut report = Report {
        menu_command_not_in_package: Vec::new(),
        command_in_menu_file_and_desktop_file: Vec::new(),
        menu_item_creates_new_section: Vec::new(),
        desktop_command_not_in_package: Vec::new(),
        desktop_entry_contains_unknown_key: Vec::new(),
        menu_item_uses_apps_section: Vec::new(),
        menu_item_uses_windowmanagers_section: Vec::new(),
        non_wm_in_windowmanager_menu_section: Vec::new(),
        non_wm_module_in_wm_modules_menu_section: Vec::new(),
        menu_item_needs_tag_has_unknown_value: Vec::new(),
        desktop_entry_file_has_crs: Vec::new(),
        desktop_entry_invalid_category: Vec::new(),
        desktop_entry_uses_reserved_category: Vec::new(),
        pkg_not_in_package_test: Vec::new(),
        unquoted_string_in_menu_item: Vec::new(),
        su_wrapper_without_c: Vec::new(),
        su_wrapper_not_su_to_root: Vec::new(),
        menu_icon_uses_relative_path: Vec::new(),
        menu_icon_missing: Vec::new(),
        menu_item_missing_required_tag: Vec::new(),
        su_to_root_with_usr_sbin: Vec::new(),
        bad_test_in_menu_item: Vec::new(),
        duplicate_tag_in_menu: Vec::new(),
        desktop_contains_deprecated_key: Vec::new(),
        desktop_entry_contains_deprecated_key: Vec::new(),
        desktop_entry_contains_encoding_key: Vec::new(),
        desktop_entry_lacks_keywords_entry: Vec::new(),
        duplicate_key_in_desktop: Vec::new(),
        desktop_entry_unknown_type: Vec::new(),
        desktop_entry_missing_required_key: Vec::new(),
        desktop_entry_lacks_main_category: Vec::new(),
        desktop_mime_but_no_exec_code: Vec::new(),
        menu_item_contains_unknown_tag: Vec::new(),
        menu_item_creates_new_root_section: Vec::new(),
        menu_item_needs_dwww: Vec::new(),
        menu_item_uses_apps_games_section: Vec::new(),
        menu_item_uses_icon_none: Vec::new(),
        menu_icon_not_in_xpm_format: Vec::new(),
        menu_icon_too_big: Vec::new(),
        menu_icon_cannot_be_parsed: Vec::new(),
        unparsable_menu_item: Vec::new(),
        whitespace_after_continuation_character: Vec::new(),
    };
    let mut desktop_cmds = HashSet::new();
    for entry in desktop_files(files) {
        let Some(text) = read_text(files, entry) else {
            continue;
        };
        parse_desktop(
            entry.path().to_path_buf(),
            &text,
            files,
            package,
            &mut desktop_cmds,
            &mut report,
        );
    }
    for entry in menu_files(files) {
        let Some(text) = read_text(files, entry) else {
            continue;
        };
        parse_menu(
            entry.path().to_path_buf(),
            &text,
            files,
            package,
            &desktop_cmds,
            &mut report,
        );
    }
    report
}

fn warn_at(out: &mut Vec<Finding>, path: &Path, line: Option<u32>, message: impl Into<String>) {
    out.push(Finding {
        path: path.to_path_buf(),
        line,
        message: message.into(),
    });
}

fn menu_files(files: &FileTree) -> Vec<&Entry> {
    sorted_files(files, |entry| {
        let Some(path) = entry.path().to_str() else {
            return false;
        };
        let Some(name) =
            child_name(path, "usr/share/menu/").or_else(|| child_name(path, "usr/lib/menu/"))
        else {
            return false;
        };
        name != "README"
    })
}

fn desktop_files(files: &FileTree) -> Vec<&Entry> {
    sorted_files(files, |entry| {
        let Some(path) = entry.path().to_str() else {
            return false;
        };
        if path.contains("template") {
            return false;
        }
        let Some(name) = child_name(path, "usr/share/applications/")
            .or_else(|| child_name(path, "usr/share/xsessions/"))
        else {
            return false;
        };
        name.ends_with(".desktop")
    })
}

fn sorted_files(files: &FileTree, pred: impl Fn(&Entry) -> bool) -> Vec<&Entry> {
    let mut entries: Vec<_> = files
        .entries()
        .iter()
        .filter(|entry| is_file(entry) && pred(entry))
        .collect();
    entries.sort_by(|left, right| left.path().cmp(right.path()));
    entries
}

fn is_file(entry: &Entry) -> bool {
    matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink)
}

fn child_name<'a>(path: &'a str, dir: &str) -> Option<&'a str> {
    let name = path.strip_prefix(dir)?;
    if name.is_empty() || name.contains('/') {
        None
    } else {
        Some(name)
    }
}

fn read_text(files: &FileTree, entry: &Entry) -> Option<String> {
    let bytes = files.read(entry).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

fn parse_menu(
    path: PathBuf,
    text: &str,
    files: &FileTree,
    package: &str,
    desktop_cmds: &HashSet<String>,
    report: &mut Report,
) {
    let lines = physical_lines(text);
    let Some(start) = lines
        .iter()
        .position(|line| !is_blank(&line.body) && !is_comment(&line.body))
    else {
        return;
    };
    if is_menu2(&lines[start].body) {
        return;
    }
    let mut acc = String::new();
    let mut lc = 0u32;
    for line in &lines[start..] {
        lc += 1;
        if is_comment(&line.body) {
            continue;
        }
        acc.push_str(&line.stored());
        if !continues(&line.body) {
            verify_menu_line(&path, &acc, lc, files, package, desktop_cmds, report);
            acc.clear();
        }
    }
    verify_menu_line(&path, &acc, lc, files, package, desktop_cmds, report);
}

fn verify_menu_line(
    path: &Path,
    joined: &str,
    line: u32,
    files: &FileTree,
    package: &str,
    desktop_cmds: &HashSet<String>,
    report: &mut Report,
) {
    if whitespace_after_continuation(joined) {
        warn_at(
            &mut report.whitespace_after_continuation_character,
            path,
            Some(line),
            "",
        );
    }
    let line_text = normalize_menu_line(joined);
    if is_blank(&line_text) || is_comment(&line_text) {
        return;
    }
    let Some((pkg_test, rest)) = split_package_prefix(&line_text) else {
        warn_at(&mut report.bad_test_in_menu_item, path, Some(line), "");
        return;
    };
    let tested = tested_packages(pkg_test);
    if !tested.iter().any(|name| name == package) {
        warn_at(
            &mut report.pkg_not_in_package_test,
            path,
            Some(line),
            pkg_test,
        );
    }
    if !lintian_parses(rest) {
        warn_at(&mut report.unparsable_menu_item, path, Some(line), "");
    }
    let (pairs, parsed) = parse_pairs(rest);
    let mut seen_tags = HashSet::new();
    for (tag, _, quoted) in &pairs {
        if !seen_tags.insert(tag.as_str()) {
            warn_at(
                &mut report.duplicate_tag_in_menu,
                path,
                Some(line),
                tag.clone(),
            );
        }
        if !quoted {
            warn_at(
                &mut report.unquoted_string_in_menu_item,
                path,
                Some(line),
                tag.clone(),
            );
        }
    }
    if !parsed {
        return;
    }
    let mut vals: HashMap<String, String> = HashMap::new();
    for (tag, value, _) in pairs {
        vals.insert(tag, value);
    }
    if let Some(tag) = ["needs", "section", "title", "command"]
        .into_iter()
        .find(|tag| !vals.contains_key(*tag))
    {
        warn_at(
            &mut report.menu_item_missing_required_tag,
            path,
            Some(line),
            tag.to_string(),
        );
        return;
    }
    let mut unknown: Vec<&String> = vals
        .keys()
        .filter(|tag| !KNOWN_MENU_TAGS.contains(&tag.as_str()))
        .collect();
    unknown.sort();
    for tag in unknown {
        warn_at(
            &mut report.menu_item_contains_unknown_tag,
            path,
            Some(line),
            tag.clone(),
        );
    }
    let original_section = vals["section"].clone();
    let section = squeeze_slashes(&original_section);
    let needs = vals["needs"].to_lowercase();
    let cmd = verify_cmd(files, &vals["command"]);
    note_su_wrapper(&cmd, path, Some(line), package, report);
    for (tag, name) in menu_icons(&vals) {
        warn_at(
            &mut report.menu_icon_uses_relative_path,
            path,
            Some(line),
            format!("{tag} {name}"),
        );
    }
    for (tag, name) in missing_menu_icons(files, &vals) {
        warn_at(
            &mut report.menu_icon_missing,
            path,
            Some(line),
            format!("{tag} {name}"),
        );
    }
    icon_checks(files, &vals, path, Some(line), report);
    if !cmd.okay
        && cmd.file.as_ref().is_some_and(|command| !command.is_empty())
        && tested.len() < 2
        && !is_modules_section(&section)
    {
        warn_at(
            &mut report.menu_command_not_in_package,
            path,
            Some(line),
            cmd.file.clone().unwrap_or_default(),
        );
    }
    if let Some(command) = cmd.file.as_ref().filter(|command| !command.is_empty()) {
        let key = menu_command_key(command);
        if desktop_cmds.contains(&key) {
            warn_at(
                &mut report.command_in_menu_file_and_desktop_file,
                path,
                Some(line),
                key,
            );
        }
    }
    if is_modules_section(&section) {
        if matches!(needs.as_str(), "x11" | "text" | "vc" | "wm") {
            warn_at(
                &mut report.non_wm_module_in_wm_modules_menu_section,
                path,
                Some(line),
                needs.clone(),
            );
        }
    } else if is_wm_managers_section(&section) && needs != "wm" {
        warn_at(
            &mut report.non_wm_in_windowmanager_menu_section,
            path,
            Some(line),
            needs.clone(),
        );
    } else if needs == "dwww" {
        warn_at(&mut report.menu_item_needs_dwww, path, Some(line), "");
    } else if !is_wm_managers_section(&section)
        && needs != "dwww"
        && !matches!(needs.as_str(), "x11" | "text" | "vc")
    {
        warn_at(
            &mut report.menu_item_needs_tag_has_unknown_value,
            path,
            Some(line),
            needs.clone(),
        );
    }
    let classified = classify_section(&section);
    if classified.uses_apps_games {
        warn_at(
            &mut report.menu_item_uses_apps_games_section,
            path,
            Some(line),
            "",
        );
    }
    if classified.uses_apps {
        warn_at(
            &mut report.menu_item_uses_apps_section,
            path,
            Some(line),
            "",
        );
    }
    if classified.uses_windowmanagers {
        warn_at(
            &mut report.menu_item_uses_windowmanagers_section,
            path,
            Some(line),
            "",
        );
    }
    if is_new_section(&classified.rewritten) {
        warn_at(
            &mut report.menu_item_creates_new_section,
            path,
            Some(line),
            original_section,
        );
    }
    let root = classified.rewritten.split('/').next().unwrap_or("");
    if !data::menu_root_known(root) && !root.to_lowercase().contains(&package.to_lowercase()) {
        warn_at(
            &mut report.menu_item_creates_new_root_section,
            path,
            Some(line),
            root,
        );
    }
}

fn parse_desktop(
    path: PathBuf,
    text: &str,
    files: &FileTree,
    package: &str,
    desktop_cmds: &mut HashSet<String>,
    report: &mut Report,
) {
    let mut saw_first = false;
    let mut warned_cr = false;
    let mut vals: HashMap<String, String> = HashMap::new();
    let mut unknown: Vec<(u32, String)> = Vec::new();
    let mut deprecated: Vec<(u32, String)> = Vec::new();
    let mut encoding: Vec<(u32, String)> = Vec::new();
    for (position, phys) in physical_lines(text).into_iter().enumerate() {
        let line_no = u32::try_from(position + 1).unwrap_or(u32::MAX);
        let mut line = phys.body;
        if let Some(index) = line.find('\r') {
            if !warned_cr {
                warn_at(
                    &mut report.desktop_entry_file_has_crs,
                    &path,
                    Some(line_no),
                    "",
                );
                warned_cr = true;
            }
            line.remove(index);
        }
        if is_blank(&line) || is_comment(&line) {
            continue;
        }
        if saw_first && is_group_header(&line) {
            break;
        }
        if !saw_first {
            if !is_desktop_header(&line) {
                return;
            }
            saw_first = true;
            if trim_end_ws(&line) == "[KDE Desktop Entry]" {
                warn_at(
                    &mut report.desktop_contains_deprecated_key,
                    &path,
                    Some(line_no),
                    "",
                );
            }
            continue;
        }
        let Some((tag, value)) = split_desktop_key(&line) else {
            continue;
        };
        if vals.contains_key(&tag) {
            warn_at(
                &mut report.duplicate_key_in_desktop,
                &path,
                Some(line_no),
                tag.clone(),
            );
        } else {
            let base = base_tag(&tag);
            if data::contains(data::DEPRECATED_DESKTOP_KEYS, base) {
                if base == "Encoding" {
                    encoding.push((line_no, tag.clone()));
                } else {
                    deprecated.push((line_no, tag.clone()));
                }
            } else if !data::contains(data::KNOWN_DESKTOP_KEYS, base)
                && !data::contains(data::KDE_DESKTOP_KEYS, base)
                && !base.starts_with("X-")
            {
                unknown.push((line_no, tag.clone()));
            }
        }
        vals.insert(tag, value);
    }
    let Some(kind) = vals.get("Type") else {
        return;
    };
    if !data::contains(data::DESKTOP_TYPES, kind) {
        warn_at(
            &mut report.desktop_entry_unknown_type,
            &path,
            None,
            kind.clone(),
        );
        return;
    }
    for (line_no, tag) in deprecated {
        warn_at(
            &mut report.desktop_entry_contains_deprecated_key,
            &path,
            Some(line_no),
            tag,
        );
    }
    for (line_no, tag) in encoding {
        warn_at(
            &mut report.desktop_entry_contains_encoding_key,
            &path,
            Some(line_no),
            tag,
        );
    }
    for (line_no, tag) in unknown {
        warn_at(
            &mut report.desktop_entry_contains_unknown_key,
            &path,
            Some(line_no),
            tag,
        );
    }
    for key in ["Type", "Name"] {
        if !vals.contains_key(key) {
            warn_at(
                &mut report.desktop_entry_missing_required_key,
                &path,
                None,
                key,
            );
        }
    }
    if !vals.contains_key("NoDisplay")
        && !vals.contains_key("Keywords")
        && kind.as_str() == "Application"
    {
        warn_at(
            &mut report.desktop_entry_lacks_keywords_entry,
            &path,
            None,
            "",
        );
    }
    let in_applications = path
        .to_str()
        .is_some_and(|name| name.starts_with("usr/share/applications/"));
    if in_applications
        && vals
            .get("Exec")
            .is_some_and(|exec| exec.chars().any(|ch| !is_ws(ch)))
    {
        let exec = vals["Exec"].clone();
        let cmd = verify_cmd(files, &exec);
        note_su_wrapper(&cmd, &path, None, package, report);
        if !cmd.okay && cmd.file.as_deref() != Some("kcmshell") {
            warn_at(
                &mut report.desktop_command_not_in_package,
                &path,
                None,
                cmd.file.clone().unwrap_or_default(),
            );
        }
        let key = cmd
            .file
            .as_deref()
            .map(desktop_command_key)
            .unwrap_or_default();
        if !is_suppressed_desktop_command(&key) {
            desktop_cmds.insert(key);
        }
    }
    let only_show_in = perl_true(vals.get("OnlyShowIn").map(String::as_str));
    if let Some(categories) = vals.get("Categories") {
        let mut saw_main = false;
        for category in split_semicolons(categories) {
            if category.starts_with("X-") {
                continue;
            }
            if data::contains(data::RESERVED_CATEGORIES, category) {
                saw_main = true;
                if !only_show_in {
                    warn_at(
                        &mut report.desktop_entry_uses_reserved_category,
                        &path,
                        None,
                        category,
                    );
                }
            } else if !data::contains(data::ADD_CATEGORIES, category)
                && !data::contains(data::MAIN_CATEGORIES, category)
            {
                warn_at(
                    &mut report.desktop_entry_invalid_category,
                    &path,
                    None,
                    category,
                );
            } else if data::contains(data::MAIN_CATEGORIES, category) {
                saw_main = true;
            }
        }
        if !saw_main {
            warn_at(
                &mut report.desktop_entry_lacks_main_category,
                &path,
                None,
                "",
            );
        }
    }
    if in_applications
        && vals.contains_key("MimeType")
        && !vals.get("Exec").is_some_and(|exec| has_file_code(exec))
    {
        warn_at(&mut report.desktop_mime_but_no_exec_code, &path, None, "");
    }
}

/// `(?:^|[^%])%[fFuU]`: a field code that is not an escaped `%%`.
fn has_file_code(exec: &str) -> bool {
    let bytes = exec.as_bytes();
    bytes.iter().enumerate().any(|(index, byte)| {
        *byte == b'%'
            && matches!(bytes.get(index + 1), Some(b'f' | b'F' | b'u' | b'U'))
            && (index == 0 || bytes[index - 1] != b'%')
    })
}

struct Cmd {
    okay: bool,
    /// Command with one leading `/` removed. `None` when the wrapper left it unset.
    file: Option<String>,
    /// Wrapper still named after the gksu/kdesu empty-command clearing.
    wrapper: Option<&'static str>,
    /// `verify_cmd` found a command (`-c`, a trailing gksu argument, or the bare command).
    has_command: bool,
    /// The first word is exactly `/usr/sbin/su-to-root`.
    su_to_root_with_usr_sbin: bool,
}

fn note_su_wrapper(cmd: &Cmd, path: &Path, line: Option<u32>, package: &str, report: &mut Report) {
    if cmd.su_to_root_with_usr_sbin {
        warn_at(&mut report.su_to_root_with_usr_sbin, path, line, "");
    }
    let Some(wrapper) = cmd.wrapper else {
        return;
    };
    if !cmd.has_command {
        warn_at(
            &mut report.su_wrapper_without_c,
            path,
            line,
            wrapper.to_string(),
        );
    }
    if !wrapper.contains("su-to-root") && wrapper != package {
        warn_at(
            &mut report.su_wrapper_not_su_to_root,
            path,
            line,
            wrapper.to_string(),
        );
    }
}

fn menu_icons(vals: &HashMap<String, String>) -> Vec<(&'static str, String)> {
    let mut icons = Vec::new();
    for tag in ["icon", "icon32x32", "icon16x16"] {
        let Some(name) = vals.get(tag) else {
            continue;
        };
        if name.is_empty() || name == "none" || name.starts_with('/') {
            continue;
        }
        icons.push((tag, name.clone()));
    }
    icons
}

/// Lintian's `verify_icon` for the three icon tags, except the missing check
/// (LN0505). An icon found in the package has its XPM header and size read.
fn icon_checks(
    files: &FileTree,
    vals: &HashMap<String, String>,
    path: &Path,
    line: Option<u32>,
    report: &mut Report,
) {
    for (tag, size) in [("icon", 32), ("icon32x32", 32), ("icon16x16", 16)] {
        let Some(value) = vals.get(tag) else {
            continue;
        };
        if value.is_empty() {
            continue;
        }
        if value == "none" {
            warn_at(&mut report.menu_item_uses_icon_none, path, line, tag);
            continue;
        }
        let name = value.trim_start_matches('/');
        if !name.to_ascii_lowercase().ends_with(".xpm") {
            warn_at(
                &mut report.menu_icon_not_in_xpm_format,
                path,
                line,
                format!("{tag} {name}"),
            );
            continue;
        }
        let pixmap = format!("usr/share/pixmaps/{name}");
        let Some(icon) = resolve_entry(files, name).or_else(|| resolve_entry(files, &pixmap))
        else {
            continue;
        };
        if !is_file(icon) {
            continue;
        }
        let text = files
            .read(icon)
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .unwrap_or_default();
        match xpm_size(&text) {
            Err(step) => warn_at(
                &mut report.menu_icon_cannot_be_parsed,
                path,
                line,
                format!("{tag} {name}: looking for {step}"),
            ),
            Ok((width, height)) => {
                if width > size || height > size {
                    warn_at(
                        &mut report.menu_icon_too_big,
                        path,
                        line,
                        format!("{tag} {name}: {width}x{height} > {size}x{size}"),
                    );
                }
            }
        }
    }
}

/// The width and height on the XPM size line, after the `/* XPM */` header.
/// `Err` names the step that found no match.
fn xpm_size(text: &str) -> Result<(u64, u64), &'static str> {
    let header = Regex::new(r"/\*\s*XPM\s*\*/").expect("valid regex");
    let size = Regex::new(r#""\s*([0-9]+)\s*([0-9]+)\s*(?:[0-9]+)\s*(?:[0-9]+)\s*""#)
        .expect("valid regex");
    let mut lines = text.split_inclusive('\n');
    if !lines.any(|line| header.is_match(line)) {
        return Err("XPM header");
    }
    let Some(caps) = lines.find_map(|line| size.captures(line)) else {
        return Err("size line");
    };
    let width = caps[1].parse::<u64>().unwrap_or(u64::MAX);
    let height = caps[2].parse::<u64>().unwrap_or(u64::MAX);
    Ok((width, height))
}

/// `.xpm` icons lintian would report missing. Leading slashes are already gone
/// from the returned path. `none` is decided before that strip.
fn missing_menu_icons(
    files: &FileTree,
    vals: &HashMap<String, String>,
) -> Vec<(&'static str, String)> {
    let mut missing = Vec::new();
    for tag in ["icon", "icon32x32", "icon16x16"] {
        let Some(name) = vals.get(tag) else {
            continue;
        };
        if name.is_empty() || name == "none" {
            continue;
        }
        let stripped = name.trim_start_matches('/');
        if !stripped.to_ascii_lowercase().ends_with(".xpm") {
            continue;
        }
        if !icon_is_file(files, stripped) {
            missing.push((tag, stripped.to_string()));
        }
    }
    missing
}

fn icon_is_file(files: &FileTree, name: &str) -> bool {
    resolves_to_file(files, name) || resolves_to_file(files, &format!("usr/share/pixmaps/{name}"))
}

/// Lintian's `resolve_path`: the entry at `path` after following symlinks,
/// whatever its kind. `None` when the path is missing or a link is broken.
fn resolve_entry<'a>(files: &'a FileTree, path: &str) -> Option<&'a Entry> {
    let mut current = lexical_join(Path::new(""), Path::new(path));
    for _ in 0..8 {
        let entry = files.get(&current)?;
        match entry.kind() {
            EntryKind::Symlink => {
                let target = entry.link_target()?;
                let base = if target.is_absolute() {
                    Path::new("")
                } else {
                    current.parent().unwrap_or(Path::new(""))
                };
                current = lexical_join(base, target);
            }
            _ => return Some(entry),
        }
    }
    None
}

/// Lintian's `resolve_path` plus `is_open_ok`: a regular file or hardlink,
/// following symlinks. A directory and a broken link are not openable.
fn resolves_to_file(files: &FileTree, path: &str) -> bool {
    let mut current = lexical_join(Path::new(""), Path::new(path));
    for _ in 0..8 {
        let Some(entry) = files.get(&current) else {
            return false;
        };
        match entry.kind() {
            EntryKind::RegularFile | EntryKind::Hardlink => return true,
            EntryKind::Directory
            | EntryKind::Fifo
            | EntryKind::CharDevice
            | EntryKind::BlockDevice => return false,
            EntryKind::Symlink => {
                let Some(target) = entry.link_target() else {
                    return false;
                };
                let base = if target.is_absolute() {
                    Path::new("")
                } else {
                    current.parent().unwrap_or(Path::new(""))
                };
                current = lexical_join(base, target);
            }
        }
    }
    false
}

fn lexical_join(base: &Path, target: &Path) -> PathBuf {
    let mut joined = base.to_path_buf();
    for component in target.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                joined.pop();
            }
            Component::Normal(part) => joined.push(part),
            Component::RootDir => joined = PathBuf::new(),
            Component::Prefix(_) => {}
        }
    }
    joined
}

fn verify_cmd(files: &FileTree, exec: &str) -> Cmd {
    let mut components = split_fields(exec);
    let su_to_root_with_usr_sbin = components.first() == Some(&"/usr/sbin/su-to-root");
    let mut wrapper_left = None;
    let file = if components
        .first()
        .is_some_and(|command| wrapper_name(command).is_some())
    {
        let wrapper = wrapper_name(components.remove(0)).expect("wrapper was checked");
        wrapper_left = Some(wrapper);
        let mut cmd = None;
        while let Some(current) = components.first().copied() {
            if current.is_empty() {
                components.remove(0);
                continue;
            }
            if current == "-c" {
                cmd = components.get(1).map(|value| (*value).to_string());
                break;
            } else if takes_argument(current) {
                components.remove(0);
                if !components.is_empty() {
                    components.remove(0);
                }
            } else if current.starts_with('-') {
                components.remove(0);
            } else {
                break;
            }
        }
        if cmd.is_none() && (wrapper == "gksu" || wrapper == "kdesu") {
            cmd = Some(if let Some(value) = components.first() {
                (*value).to_string()
            } else {
                wrapper_left = None;
                wrapper.to_string()
            });
        }
        cmd
    } else {
        components.first().map(|value| (*value).to_string())
    };
    let has_command = file.as_ref().is_some_and(|command| !command.is_empty());
    let stored = file
        .as_ref()
        .map(|command| command.strip_prefix('/').unwrap_or(command).to_string());
    let okay = file.as_ref().is_some_and(|command| {
        command.starts_with(['\'', '"'])
            || stored
                .as_ref()
                .is_some_and(|path| files.get(path.as_str()).is_some())
            || is_shell(command)
            || is_sensible(command)
            || COMMAND_PATH
                .iter()
                .any(|dir| files.get(format!("{dir}{command}")).is_some())
    });
    Cmd {
        okay,
        file: stored,
        wrapper: wrapper_left,
        has_command,
        su_to_root_with_usr_sbin,
    }
}

const COMMAND_PATH: &[&str] = &["usr/local/bin/", "usr/bin/", "bin/", "usr/games/"];

fn wrapper_name(command: &str) -> Option<&'static str> {
    let bare = command
        .strip_prefix("/usr/bin/")
        .or_else(|| command.strip_prefix("/usr/sbin/"))
        .unwrap_or(command);
    match bare {
        "su-to-root" => Some("su-to-root"),
        "gksu" => Some("gksu"),
        "kdesu" => Some("kdesu"),
        "sux" => Some("sux"),
        _ => None,
    }
}

fn takes_argument(option: &str) -> bool {
    let bytes = option.as_bytes();
    if bytes.len() >= 2 && bytes[0] == b'-' && bytes[1] != b'-' {
        return matches!(bytes[1], b'D' | b'f' | b'm' | b'u' | b'p' | b'i');
    }
    option.starts_with("--user")
        || option.starts_with("--description")
        || option.starts_with("--message")
}

fn is_shell(command: &str) -> bool {
    let rest = if let Some(rest) = command.strip_prefix("/bin/") {
        rest
    } else if command.starts_with('/') {
        return false;
    } else {
        command
    };
    rest.starts_with("sh")
}

fn is_sensible(command: &str) -> bool {
    let rest = if let Some(rest) = command.strip_prefix("/usr/bin/") {
        rest
    } else if command.starts_with('/') {
        return false;
    } else {
        command
    };
    rest.starts_with("sensible-pager")
        || rest.starts_with("sensible-editor")
        || rest.starts_with("sensible-browser")
}

fn desktop_command_key(command: &str) -> String {
    strip_bin(command).to_string()
}

fn menu_command_key(command: &str) -> String {
    strip_games(strip_bin(command)).to_string()
}

fn strip_bin(command: &str) -> &str {
    for prefix in ["usr/sbin/", "usr/bin/", "sbin/", "bin/"] {
        if let Some(rest) = command.strip_prefix(prefix) {
            return rest;
        }
    }
    command
}

fn strip_games(command: &str) -> &str {
    command.strip_prefix("usr/games/").unwrap_or(command)
}

fn is_suppressed_desktop_command(command: &str) -> bool {
    matches!(command, "su-to-root" | "su" | "sux" | "gksu" | "kdesu")
}

fn is_modules_section(section: &str) -> bool {
    section.starts_with("WindowManagers/Modules")
        || section.starts_with("FVWM Modules")
        || section.starts_with("Window Maker")
}

fn is_wm_managers_section(section: &str) -> bool {
    !is_modules_section(section)
        && (section.starts_with("WindowManagers") || section.starts_with("Window Managers"))
}

struct ClassifiedSection {
    rewritten: String,
    uses_apps_games: bool,
    uses_apps: bool,
    uses_windowmanagers: bool,
}

fn classify_section(section: &str) -> ClassifiedSection {
    let mut rewritten = section.to_string();
    let uses_apps_games = rewritten.starts_with("Apps/Games");
    if rewritten.starts_with("Apps/Games") {
        rewritten.replace_range(0.."Apps/".len(), "");
    }
    let uses_apps = rewritten.starts_with("Apps/");
    if uses_apps {
        rewritten.replace_range(0.."Apps/".len(), "Applications/");
    }
    let uses_windowmanagers = rewritten.starts_with("WindowManagers");
    if uses_windowmanagers {
        rewritten.replace_range(0.."WindowManagers".len(), "Window Managers");
    }
    ClassifiedSection {
        rewritten,
        uses_apps_games,
        uses_apps,
        uses_windowmanagers,
    }
}

fn is_new_section(section: &str) -> bool {
    let (root, under) = section.split_once('/').unwrap_or((section, ""));
    data::menu_root_known(root) && !data::menu_under_known(root, under)
}

fn squeeze_slashes(section: &str) -> String {
    let mut out = String::with_capacity(section.len());
    let mut prev_slash = false;
    for ch in section.chars() {
        if ch == '/' {
            if !prev_slash {
                out.push('/');
            }
            prev_slash = true;
        } else {
            prev_slash = false;
            out.push(ch);
        }
    }
    if out != "/" && out.ends_with('/') {
        out.pop();
    }
    out
}

fn tested_packages(test: &str) -> Vec<String> {
    if test.is_empty() {
        return Vec::new();
    }
    let raw: Vec<&str> = test.split(',').collect();
    let last = raw.len() - 1;
    let mut parts: Vec<String> = raw
        .iter()
        .enumerate()
        .map(|(index, part)| {
            let part = if index > 0 {
                trim_start_ws(part)
            } else {
                *part
            };
            if index < last {
                trim_end_ws(part)
            } else {
                part
            }
            .to_string()
        })
        .collect();
    while parts.last().is_some_and(|part| part.is_empty()) {
        parts.pop();
    }
    let mut unique = Vec::new();
    let mut seen = HashSet::new();
    for part in parts {
        if seen.insert(part.clone()) {
            unique.push(part);
        }
    }
    unique
}

fn split_package_prefix(line: &str) -> Option<(&str, &str)> {
    let rest = line.strip_prefix("?package(")?;
    let end = rest.find("):")?;
    Some((&rest[..end], &rest[end + 2..]))
}

/// Pairs read before the parse stopped, and whether the whole line parsed.
/// Lintian reports duplicate and unquoted tags while it scans, so a line it
/// later calls unparsable still yields them.
fn parse_pairs(line: &str) -> (Vec<(String, String, bool)>, bool) {
    let chars: Vec<char> = line.chars().collect();
    let mut index = 0;
    let mut pairs = Vec::new();
    while index < chars.len() {
        while index < chars.len() && is_ws(chars[index]) {
            index += 1;
        }
        if index >= chars.len() {
            break;
        }
        let tag_start = index;
        while index < chars.len()
            && !is_ws(chars[index])
            && chars[index] != '"'
            && chars[index] != '='
        {
            index += 1;
        }
        if index >= chars.len() || chars[index] != '=' {
            return (pairs, false);
        }
        let tag: String = chars[tag_start..index].iter().collect();
        index += 1;
        let (next, raw) = if chars.get(index) == Some(&'"') {
            match take_quoted(&chars, index) {
                Some(taken) => taken,
                None => {
                    pairs.push((tag, String::new(), false));
                    return (pairs, false);
                }
            }
        } else {
            take_unquoted(&chars, index)
        };
        index = next;
        let quoted = raw.starts_with('"');
        pairs.push((tag, decode_value(&raw), quoted));
    }
    (pairs, true)
}

/// Whether lintian's `verify_line` loop consumes the whole item. Lintian scans
/// with an unanchored `/gc` match, so a bare token before a later `tag=value`
/// is skipped; only non-blank text after the last pair makes the item
/// unparsable. (`parse_pairs` stops at such a token and serves the other Rules.)
fn lintian_parses(rest: &str) -> bool {
    static PAIR: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
        Regex::new(r#"\s*?([^"\s=]*)=("(?:\\.|[^"])*"|[^"\s]*)"#).expect("valid regex")
    });
    let mut pos = 0;
    while let Some(found) = PAIR.find_at(rest, pos) {
        pos = found.end();
    }
    trim_start_ws(&rest[pos..]).is_empty()
}

fn take_quoted(chars: &[char], start: usize) -> Option<(usize, String)> {
    let mut index = start + 1;
    let mut raw = String::from("\"");
    while index < chars.len() {
        if chars[index] == '\\' && index + 1 < chars.len() {
            raw.push('\\');
            raw.push(chars[index + 1]);
            index += 2;
            continue;
        }
        if chars[index] == '"' {
            raw.push('"');
            return Some((index + 1, raw));
        }
        raw.push(chars[index]);
        index += 1;
    }
    None
}

fn take_unquoted(chars: &[char], start: usize) -> (usize, String) {
    let mut index = start;
    while index < chars.len() && chars[index] != '"' && !is_ws(chars[index]) {
        index += 1;
    }
    (index, chars[start..index].iter().collect())
}

fn decode_value(raw: &str) -> String {
    let inner = if raw.len() >= 2 && raw.starts_with('"') && raw.ends_with('"') {
        &raw[1..raw.len() - 1]
    } else {
        raw
    };
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some(next) => out.push(next),
                None => out.push('\\'),
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn split_desktop_key(line: &str) -> Option<(String, String)> {
    let eq = line.find('=')?;
    Some((
        trim_end_ws(&line[..eq]).to_string(),
        trim_start_ws(&line[eq + 1..]).to_string(),
    ))
}

fn base_tag(tag: &str) -> &str {
    let Some(open) = tag.rfind('[') else {
        return tag;
    };
    if !tag.ends_with(']') || open + 1 >= tag.len() - 1 {
        return tag;
    }
    let inner = &tag[open + 1..tag.len() - 1];
    if inner.is_empty() || inner.contains(']') {
        tag
    } else {
        &tag[..open]
    }
}

fn split_semicolons(value: &str) -> Vec<&str> {
    if value.is_empty() {
        return Vec::new();
    }
    let mut parts: Vec<&str> = value.split(';').collect();
    while parts.last().is_some_and(|part| part.is_empty()) {
        parts.pop();
    }
    parts
}

fn perl_true(value: Option<&str>) -> bool {
    matches!(value, Some(text) if !text.is_empty() && text != "0")
}

fn is_group_header(line: &str) -> bool {
    let line = trim_end_ws(line);
    line.starts_with('[') && line.ends_with(']') && line.len() >= 2
}

fn is_desktop_header(line: &str) -> bool {
    let line = trim_end_ws(line);
    line == "[Desktop Entry]" || line == "[KDE Desktop Entry]"
}

fn is_menu2(body: &str) -> bool {
    let Some(rest) = body.strip_prefix("!C") else {
        return false;
    };
    trim_start_ws(rest).starts_with("menu-2")
}

fn is_comment(body: &str) -> bool {
    trim_start_ws(body).starts_with('#')
}

fn is_blank(body: &str) -> bool {
    body.chars().all(is_ws)
}

fn continues(body: &str) -> bool {
    trim_end_ws(body).ends_with('\\')
}

struct PhysLine {
    body: String,
    had_nl: bool,
}

impl PhysLine {
    fn stored(&self) -> String {
        if self.had_nl {
            format!("{}\n", self.body)
        } else {
            self.body.clone()
        }
    }
}

fn physical_lines(text: &str) -> Vec<PhysLine> {
    let mut lines = Vec::new();
    let mut rest = text;
    while let Some(index) = rest.find('\n') {
        lines.push(PhysLine {
            body: rest[..index].to_string(),
            had_nl: true,
        });
        rest = &rest[index + 1..];
    }
    if !rest.is_empty() {
        lines.push(PhysLine {
            body: rest.to_string(),
            had_nl: false,
        });
    }
    lines
}

/// Lintian's `s/\\\s+\n/ /` after its plain `\`-newline join: a backslash,
/// whitespace, then a newline inside one item. The trailing newline is chomped first.
fn whitespace_after_continuation(joined: &str) -> bool {
    static STRAY: std::sync::LazyLock<Regex> =
        std::sync::LazyLock::new(|| Regex::new(r"\\\s+\n").expect("valid regex"));
    let text = joined.strip_suffix('\n').unwrap_or(joined);
    STRAY.is_match(&text.replace("\\\n", " "))
}

fn normalize_menu_line(joined: &str) -> String {
    let mut text = joined.to_string();
    if text.ends_with('\n') {
        text.pop();
    }
    apply_continuations(&text)
}

fn apply_continuations(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\\' {
            if bytes.get(index + 1) == Some(&b'\n') {
                out.push(b' ');
                index += 2;
                continue;
            }
            let mut end = index + 1;
            while end < bytes.len() && is_horiz_ws(bytes[end]) {
                end += 1;
            }
            if end > index + 1 && bytes.get(end) == Some(&b'\n') {
                out.push(b' ');
                index = end + 1;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8(out)
        .unwrap_or_else(|err| String::from_utf8_lossy(err.as_bytes()).into_owned())
}

fn is_horiz_ws(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\r' | 0x0c)
}

fn is_ws(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\n' | '\r' | '\u{000c}')
}

fn trim_start_ws(text: &str) -> &str {
    text.trim_start_matches(is_ws)
}

fn trim_end_ws(text: &str) -> &str {
    text.trim_end_matches(is_ws)
}

fn split_fields(text: &str) -> Vec<&str> {
    text.split(is_ws)
        .filter(|field| !field.is_empty())
        .collect()
}

#[cfg(test)]
pub(super) mod fixture {
    /// `menu-format-general`'s `menu-section.menu`, installed as `usr/share/menu/menu-section`.
    pub const MENU_SECTION: &str = "\
?package(menu-section):needs=\"X11\" section=\"Root\"\\\n\
  title=\"pargs (new root)\" command=\"/usr/bin/pargs-section\"\\\n\
  longtitle=\"Argument printer\"\n\
?package(menu-section):needs=\"X11\" section=\"Apps/Games/Toys\"\\\n\
  title=\"pargs (apps/games)\" command=\"/usr/bin/pargs-section\"\\\n\
  longtitle=\"Argument printer\"\n\
?package(menu-section):needs=\"X11\" section=\"Games\"\\\n\
  title=\"pargs (games)\" command=\"/usr/bin/pargs-section\"\\\n\
  longtitle=\"Argument printer\"\n\
?package(menu-section):needs=\"X11\" section=\"Apps/Programming\"\\\n\
  title=\"pargs (apps/prog)\" command=\"/usr/bin/pargs-section\"\\\n\
  longtitle=\"Argument printer\"\n\
?package(menu-section):needs=\"X11\" section=\"WindowManagers/Programming\"\\\n\
  title=\"pargs (vm)\" command=\"/usr/bin/pargs-section\"\\\n\
  longtitle=\"Argument printer\"\n";

    pub const GENERAL_DESKTOP: &str = "\
[Desktop Entry]\n\
Name=general\n\
Name=general\n\
Comment=Various general errors\n\
Type=Application\n\
Foo=Bar\n\
Encoding=UTF-8\n\
Categories=Building;Lintian;\n\
Terminal=true\n\
TerminalOptions=-g 80x25\n\
Exec=foo\n\
#missing Icon\n\
#missing Keywords\n";
}
