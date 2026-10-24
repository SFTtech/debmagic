/// A process as the Environment registry records it: the pid plus its start
/// time, so a reused pid does not pass for the recorded process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Process {
    pub pid: u32,
    /// `None` when unreadable (non-Linux, or the process is already gone).
    pub start: Option<i64>,
}

impl Process {
    pub fn current() -> Self {
        Self::of(std::process::id())
    }

    pub fn of(pid: u32) -> Self {
        Self {
            pid,
            start: start_time(pid),
        }
    }

    /// Whether the recorded process still runs. Falls back to plain liveness
    /// when the start time cannot be compared.
    pub fn is_alive(self) -> bool {
        if !pid_is_alive(self.pid) {
            return false;
        }
        match (self.start, start_time(self.pid)) {
            (Some(recorded), Some(current)) => recorded == current,
            _ => true,
        }
    }
}

fn pid_is_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    let result = unsafe { libc::kill(pid as i32, 0) };
    if result == 0 {
        return true;
    }
    std::io::Error::last_os_error()
        .raw_os_error()
        .is_none_or(|code| code != libc::ESRCH)
}

/// Field 22 of `/proc/<pid>/stat`.
fn start_time(pid: u32) -> Option<i64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // comm (field 2) is parenthesized and may itself contain spaces or
    // parens; the remaining fields start after the last ')'.
    let after_comm = stat.rsplit_once(')')?.1;
    // starttime is field 22 overall, i.e. index 19 after comm.
    after_comm.split_whitespace().nth(19)?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_process_is_alive() {
        assert!(Process::current().is_alive());
    }

    #[test]
    fn exited_process_is_not_alive() {
        let mut child = std::process::Command::new("true").spawn().unwrap();
        let process = Process::of(child.id());
        child.wait().unwrap();
        assert!(!process.is_alive());
    }

    #[test]
    fn reused_pid_is_not_the_recorded_process() {
        let mut current = Process::current();
        current.start = current.start.map(|start| start + 1);
        assert!(!current.is_alive());
    }
}
