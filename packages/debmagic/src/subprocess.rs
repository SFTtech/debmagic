use std::io::Write;
use std::process::{Child, Command, Stdio};

/// Which output streams of a command run are captured into its
/// [`CommandResult`] instead of passing through to the user's terminal.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Capture {
    pub stdout: bool,
    pub stderr: bool,
}

impl Capture {
    pub const NONE: Self = Self {
        stdout: false,
        stderr: false,
    };
    pub const STDOUT: Self = Self {
        stdout: true,
        stderr: false,
    };
    pub const STDERR: Self = Self {
        stdout: false,
        stderr: true,
    };
    pub const ALL: Self = Self {
        stdout: true,
        stderr: true,
    };
}

/// Result of one command run, like Python's `CompletedProcess`: the exit
/// code, plus each stream the caller asked to capture via [`Capture`].
#[derive(Debug, Clone)]
pub struct CommandResult {
    pub exit_code: i32,
    /// Captured stdout; `None` unless the caller captured it.
    pub stdout: Option<String>,
    /// Captured stderr; `None` unless the caller captured it.
    pub stderr: Option<String>,
}

/// Pipe the streams selected by `capture`; the rest stay inherited.
fn pipe_streams(command: &mut Command, capture: Capture) {
    if capture.stdout {
        command.stdout(Stdio::piped());
    }
    if capture.stderr {
        command.stderr(Stdio::piped());
    }
}

/// Collect the piped streams of a finished child into a [`CommandResult`].
fn finish(child: Child, capture: Capture) -> std::io::Result<CommandResult> {
    let output = child.wait_with_output()?;
    Ok(CommandResult {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: capture
            .stdout
            .then(|| String::from_utf8_lossy(&output.stdout).into_owned()),
        stderr: capture
            .stderr
            .then(|| String::from_utf8_lossy(&output.stderr).into_owned()),
    })
}

/// Start building a subprocess run of `command`: pick the streams to
/// [`capture`](CommandBuilder::capture), optionally provide
/// [`input`](CommandBuilder::input), then [`run`](CommandBuilder::run) it
/// to completion or [`spawn`](CommandBuilder::spawn) it and handle the
/// child yourself.
///
/// This is the opt-in capture API: `Command::output()` pipes *every*
/// stream left at its default, silently swallowing uncaptured output,
/// while here uncaptured streams stay inherited and pass through to
/// the user's terminal.
pub fn command(command: Command) -> CommandBuilder<'static> {
    CommandBuilder {
        command,
        input: None,
        capture: Capture::NONE,
    }
}

/// A [`Command`] configured through [`command`], waiting to be run.
pub struct CommandBuilder<'a> {
    command: Command,
    input: Option<&'a [u8]>,
    capture: Capture,
}

impl CommandBuilder<'_> {
    /// Write `input` to the command's piped stdin; without it the
    /// parent's stdin is inherited.
    ///
    /// The whole input is written before the child is returned, so a
    /// command that answers on stdout before consuming all of stdin can
    /// deadlock once its output outgrows the pipe buffer.
    pub fn input<'b>(self, input: &'b [u8]) -> CommandBuilder<'b> {
        CommandBuilder {
            input: Some(input),
            ..self
        }
    }

    /// Capture the selected streams into the [`CommandResult`] instead
    /// of passing them through to the user's terminal.
    pub fn capture(mut self, capture: Capture) -> Self {
        self.capture = capture;
        self
    }

    /// Spawn the command and wait for it, collecting the captured
    /// streams.
    pub fn run(self) -> std::io::Result<CommandResult> {
        let Self {
            command,
            input,
            capture,
        } = self;
        finish(spawn(command, input, capture)?, capture)
    }

    /// Spawn the command and return the running child, so the caller
    /// can stream its output while it runs.
    pub fn spawn(self) -> std::io::Result<Child> {
        let Self {
            command,
            input,
            capture,
        } = self;
        spawn(command, input, capture)
    }
}

fn spawn(mut command: Command, input: Option<&[u8]>, capture: Capture) -> std::io::Result<Child> {
    if input.is_some() {
        command.stdin(Stdio::piped());
    }
    pipe_streams(&mut command, capture);
    let mut child = command.spawn()?;
    if let Some(input) = input {
        child
            .stdin
            .take()
            .expect("stdin is piped")
            .write_all(input)?;
    }
    Ok(child)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uncaptured_streams_pass_through() {
        let result = command(Command::new("true")).run().unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout, None);
        assert_eq!(result.stderr, None);
    }

    #[test]
    fn input_is_written_to_stdin() {
        let result = command(Command::new("cat"))
            .input(b"piped")
            .capture(Capture::STDOUT)
            .run()
            .unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout.as_deref(), Some("piped"));
    }

    #[test]
    fn stdout_capture_collects_only_stdout() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "echo out; echo err >&2"]);
        let result = command(cmd).capture(Capture::STDOUT).run().unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout.as_deref(), Some("out\n"));
        assert_eq!(result.stderr, None);
    }

    #[test]
    fn stderr_capture_collects_only_stderr() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "echo out; echo err >&2"]);
        let result = command(cmd).capture(Capture::STDERR).run().unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout, None);
        assert_eq!(result.stderr.as_deref(), Some("err\n"));
    }

    #[test]
    fn all_capture_collects_both_streams() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "echo out; echo err >&2"]);
        let result = command(cmd).capture(Capture::ALL).run().unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout.as_deref(), Some("out\n"));
        assert_eq!(result.stderr.as_deref(), Some("err\n"));
    }

    #[test]
    fn exit_code_is_reported_regardless_of_capture() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "exit 3"]);
        let result = command(cmd).capture(Capture::STDOUT).run().unwrap();
        assert_eq!(result.exit_code, 3);
    }

    #[test]
    fn spawn_without_input_leaves_stdin_inherited() {
        // `true`, not `cat`: inherited stdin is the test runner's, which
        // never reaches EOF, so a reading child would hang forever
        let mut child = command(Command::new("true")).spawn().unwrap();
        assert!(child.stdin.is_none());
        assert!(child.stdout.is_none());
        assert!(child.stderr.is_none());
        assert!(child.wait().unwrap().success());
    }

    #[test]
    fn spawn_with_input_leaves_uncaptured_streams_inherited() {
        let mut child = command(Command::new("cat"))
            .input(b"x")
            .spawn()
            .unwrap();
        assert!(child.stdout.is_none());
        assert!(child.stderr.is_none());
        assert!(child.wait().unwrap().success());
    }
}
