//! Unix-socket attach/detach tracking for build environments.
//!
//! While a build keeps its environment alive after a failure, a small
//! socket server in the build root lets concurrent `debmagic shell`
//! sessions register themselves, so the environment is only torn down
//! once the last attached shell detaches.

use core::time;
use std::net::Shutdown;
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::{Arc, Mutex};
use std::{
    fs, io,
    io::{Read, Write},
    path::{Path, PathBuf},
    thread,
};

use anyhow::Context;

fn socket_path_for_build(build_root: &Path) -> PathBuf {
    build_root.join("build.sock")
}

pub fn start_socket_server(
    build_root: &Path,
    should_exit: Arc<Mutex<bool>>,
) -> anyhow::Result<thread::JoinHandle<()>> {
    let sock = socket_path_for_build(build_root);
    if sock.exists() {
        // try to remove stale socket file
        let _ = fs::remove_file(&sock);
    }

    let listener = UnixListener::bind(&sock)
        .with_context(|| format!("failed to bind unix socket {}", sock.display()))?;

    // Set non-blocking mode so we can check the exit flag
    listener
        .set_nonblocking(true)
        .context("failed to set socket non-blocking")?;

    let handle = thread::spawn(move || {
        let mut num_attached = 0u64;
        loop {
            // Check if we should exit
            let exit_requested = *should_exit.lock().unwrap();
            if exit_requested && num_attached == 0 {
                break;
            }

            match listener.accept() {
                Ok((mut s, _)) => {
                    let mut buf = String::new();
                    if s.read_to_string(&mut buf).is_err() {
                        let _ = s.shutdown(Shutdown::Both);
                        continue;
                    }
                    let cmd = buf.trim();
                    match cmd {
                        "attach" => {
                            num_attached += 1;
                        }
                        "detach" => {
                            num_attached = num_attached.saturating_sub(1);
                        }
                        _ => {}
                    }
                    let _ = s.shutdown(Shutdown::Both);
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // No connection available, sleep briefly to avoid busy-waiting
                    thread::sleep(time::Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
        let _ = fs::remove_file(&sock);
    });

    Ok(handle)
}

pub fn send_socket_command(build_root: &Path, cmd: &str) -> anyhow::Result<()> {
    let sock = socket_path_for_build(build_root);
    let mut stream = UnixStream::connect(&sock)
        .with_context(|| format!("failed to connect to socket {}", sock.display()))?;
    stream
        .write_all(cmd.as_bytes())
        .context("failed to send socket command")?;
    stream.shutdown(Shutdown::Write).ok();
    Ok(())
}
