//! Shared menu-file and desktop-file parse for the menu-format Rules.
//!
//! Lintian reads direct children of `usr/share/menu` and `usr/lib/menu`, and
//! `.desktop` files directly under `usr/share/applications` and
//! `usr/share/xsessions`. Menu line numbers follow its counter: leading
//! comments and blank lines are not counted, and a continued item is reported
//! on its last physical line. Desktop line numbers count every physical line.
//! There is no file mode on an Entry, so executable menu files are still read.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind, FileTree};

use super::data;

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
    pub desktop_entry_file_has_crs: Vec<Finding>,
    pub desktop_entry_invalid_category: Vec<Finding>,
    pub desktop_entry_uses_reserved_category: Vec<Finding>,
    pub pkg_not_in_package_test: Vec<Finding>,
    pub unquoted_string_in_menu_item: Vec<Finding>,
    pub su_wrapper_without_c: Vec<Finding>,
    pub su_wrapper_not_su_to_root: Vec<Finding>,
    pub menu_icon_uses_relative_path: Vec<Finding>,
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
        desktop_entry_file_has_crs: Vec::new(),
        desktop_entry_invalid_category: Vec::new(),
        desktop_entry_uses_reserved_category: Vec::new(),
        pkg_not_in_package_test: Vec::new(),
        unquoted_string_in_menu_item: Vec::new(),
        su_wrapper_without_c: Vec::new(),
        su_wrapper_not_su_to_root: Vec::new(),
        menu_icon_uses_relative_path: Vec::new(),
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
    let line_text = normalize_menu_line(joined);
    if is_blank(&line_text) || is_comment(&line_text) {
        return;
    }
    let Some((pkg_test, rest)) = split_package_prefix(&line_text) else {
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
    let Some(pairs) = parse_pairs(rest) else {
        return;
    };
    for (tag, _, quoted) in &pairs {
        if !quoted {
            warn_at(
                &mut report.unquoted_string_in_menu_item,
                path,
                Some(line),
                tag.clone(),
            );
        }
    }
    let mut vals: HashMap<String, String> = HashMap::new();
    for (tag, value, _) in pairs {
        vals.insert(tag, value);
    }
    if ["needs", "section", "title", "command"]
        .iter()
        .any(|tag| !vals.contains_key(*tag))
    {
        return;
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
    if is_wm_managers_section(&section) && needs != "wm" {
        warn_at(
            &mut report.non_wm_in_windowmanager_menu_section,
            path,
            Some(line),
            needs,
        );
    }
    let classified = classify_section(&section);
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
            continue;
        }
        let Some((tag, value)) = split_desktop_key(&line) else {
            continue;
        };
        if !vals.contains_key(&tag) {
            let base = base_tag(&tag);
            if !data::contains(data::DEPRECATED_DESKTOP_KEYS, base)
                && !data::contains(data::KNOWN_DESKTOP_KEYS, base)
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
        return;
    }
    for (line_no, tag) in unknown {
        warn_at(
            &mut report.desktop_entry_contains_unknown_key,
            &path,
            Some(line_no),
            tag,
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
        for category in split_semicolons(categories) {
            if category.starts_with("X-") {
                continue;
            }
            if data::contains(data::RESERVED_CATEGORIES, category) {
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
            }
        }
    }
}

struct Cmd {
    okay: bool,
    /// Command with one leading `/` removed. `None` when the wrapper left it unset.
    file: Option<String>,
    /// Wrapper still named after the gksu/kdesu empty-command clearing.
    wrapper: Option<&'static str>,
    /// `verify_cmd` found a command (`-c`, a trailing gksu argument, or the bare command).
    has_command: bool,
}

fn note_su_wrapper(cmd: &Cmd, path: &Path, line: Option<u32>, package: &str, report: &mut Report) {
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

fn verify_cmd(files: &FileTree, exec: &str) -> Cmd {
    let mut components = split_fields(exec);
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
    uses_apps: bool,
    uses_windowmanagers: bool,
}

fn classify_section(section: &str) -> ClassifiedSection {
    let mut rewritten = section.to_string();
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

fn parse_pairs(line: &str) -> Option<Vec<(String, String, bool)>> {
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
            return None;
        }
        let tag: String = chars[tag_start..index].iter().collect();
        index += 1;
        let (next, raw) = if chars.get(index) == Some(&'"') {
            take_quoted(&chars, index)?
        } else {
            take_unquoted(&chars, index)
        };
        index = next;
        let quoted = raw.starts_with('"');
        pairs.push((tag, decode_value(&raw), quoted));
    }
    Some(pairs)
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
