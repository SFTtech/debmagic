//! Colored stage/progress output for the CLI.
//!
//! Stage banners go to **stderr** so stdout stays usable for piped command
//! output. Color is governed by a global `--color` flag (`auto`/`always`/
//! `never`, default `auto`); `auto` colors only on a TTY, and `never` is
//! implied by the `NO_COLOR` env var unless overridden with `always`.

use std::io::IsTerminal;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

pub use crate::cli::ColorChoice;

static COLOR: AtomicBool = AtomicBool::new(false);

/// An ANSI SGR style. Each variant maps to its numeric/compound code (without
/// the `\x1b[` prefix / `m` suffix), so call sites read as intent, not escape
/// soup. `Reset` clears all attributes and is emitted after styled text.
#[derive(Debug, Copy, Clone)]
#[allow(dead_code)] // the palette is a vocabulary; not every style is used yet
pub enum Style {
    Reset,
    Bold,
    Dim,
    Italic,
    Underline,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    BoldRed,
    BoldGreen,
    BoldYellow,
    BoldCyan,
}

impl Style {
    fn code(self) -> &'static str {
        match self {
            Style::Reset => "0",
            Style::Bold => "1",
            Style::Dim => "2",
            Style::Italic => "3",
            Style::Underline => "4",
            Style::Red => "31",
            Style::Green => "32",
            Style::Yellow => "33",
            Style::Blue => "34",
            Style::Magenta => "35",
            Style::Cyan => "36",
            Style::BoldRed => "1;31",
            Style::BoldGreen => "1;32",
            Style::BoldYellow => "1;33",
            Style::BoldCyan => "1;36",
        }
    }
}

/// Send a desktop notification via `notify-send`, if available. Never fails
/// the build: a headless session or missing binary just means no popup.
pub fn notify_send(summary: &str, body: &str) {
    match Command::new("notify-send")
        .arg(summary)
        .arg(body)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("notify-send not found on PATH; cannot send signing notification");
        }
        Err(e) => eprintln!("failed to run notify-send: {e}"),
    }
}

/// Ring the terminal bell so an attention-requiring prompt in a background
/// window is noticed. No-op when stderr is not a terminal, so piped output
/// stays clean.
pub fn bell() {
    use std::io::Write;
    if std::io::stderr().is_terminal() {
        let _ = std::io::stderr().write_all(b"\x07");
    }
}

/// Notify the user that their attention with both a desktop notification
/// and a terminal bell.
pub fn notify_send_bell(summary: &str, body: &str) {
    notify_send(summary, body);
    bell();
}

/// Whether color is on, after [`init_color`] has run.
pub fn color_enabled() -> bool {
    COLOR.load(Ordering::Relaxed)
}

/// Whether a `TERM` value describes a color-capable terminal, mirroring
/// strace's `is_no_color()`: no/empty TERM, or `dumb`/`unknown` (the
/// terminfo-less fallback when terminfo can't be queried) means no color.
fn term_supports_color(term: Option<&str>) -> bool {
    match term {
        None | Some("") => false,
        Some(t) => !t.eq_ignore_ascii_case("dumb") && !t.eq_ignore_ascii_case("unknown"),
    }
}

/// Pure decision, split out for testing. Color needs a TTY, a non-empty
/// `NO_COLOR` absent (no-color.org: empty `NO_COLOR=` is a no-op), and a
/// color-capable `TERM`.
fn resolve_color(
    choice: ColorChoice,
    stderr_tty: bool,
    no_color: Option<&str>,
    term: Option<&str>,
) -> bool {
    match choice {
        ColorChoice::Always => true,
        ColorChoice::Never => false,
        ColorChoice::Auto => {
            stderr_tty && no_color.is_none_or(|v| v.is_empty()) && term_supports_color(term)
        }
    }
}

/// Resolve the effective color mode and store it. Must be called once at
/// startup before any output.
pub fn init_color(choice: ColorChoice) {
    let no_color = std::env::var("NO_COLOR").ok();
    let term = std::env::var("TERM").ok();
    let enabled = resolve_color(
        choice,
        std::io::stderr().is_terminal(),
        no_color.as_deref(),
        term.as_deref(),
    );
    COLOR.store(enabled, Ordering::Relaxed);
}

/// Wrap `text` in an ANSI style (terminated by [`Style::Reset`]), or return
/// it unchanged when color is off.
pub fn styled(style: Style, text: &str) -> String {
    if color_enabled() {
        format!("\x1b[{}m{text}\x1b[{}m", style.code(), Style::Reset.code())
    } else {
        text.to_string()
    }
}

/// Print a top-level stage banner (e.g. "Building package").
pub fn stage(text: &str) {
    eprintln!("{}", styled(Style::BoldCyan, &format!("==> {text}")));
}

/// Print a sub-step line (e.g. "Installing build dependencies").
pub fn step(text: &str) {
    eprintln!("{}", styled(Style::Bold, &format!("  -> {text}")));
}

#[cfg(test)]
mod tests {
    use super::*;

    const COLOR_TERM: Option<&str> = Some("xterm-256color");

    #[test]
    fn auto_respects_tty_and_no_color() {
        // TTY, capable TERM, no NO_COLOR → color.
        assert!(resolve_color(ColorChoice::Auto, true, None, COLOR_TERM));
        // Not a TTY → no color regardless of the rest.
        assert!(!resolve_color(ColorChoice::Auto, false, None, COLOR_TERM));
        // NO_COLOR set non-empty → no color.
        assert!(!resolve_color(
            ColorChoice::Auto,
            true,
            Some("1"),
            COLOR_TERM
        ));
        // NO_COLOR set but empty → does not disable (no-color.org).
        assert!(resolve_color(ColorChoice::Auto, true, Some(""), COLOR_TERM));
    }

    #[test]
    fn auto_respects_term() {
        assert!(!resolve_color(ColorChoice::Auto, true, None, None));
        assert!(!resolve_color(ColorChoice::Auto, true, None, Some("")));
        assert!(!resolve_color(ColorChoice::Auto, true, None, Some("dumb")));
        assert!(!resolve_color(ColorChoice::Auto, true, None, Some("DUMB")));
        assert!(!resolve_color(
            ColorChoice::Auto,
            true,
            None,
            Some("unknown")
        ));
    }

    #[test]
    fn explicit_choices_override_env_and_tty() {
        assert!(resolve_color(
            ColorChoice::Always,
            false,
            Some("1"),
            Some("dumb")
        ));
        assert!(!resolve_color(ColorChoice::Never, true, None, COLOR_TERM));
    }
}
