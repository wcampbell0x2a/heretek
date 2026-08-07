use std::fs::File;
use std::io::Read;
use std::os::fd::OwnedFd;
use std::sync::{Arc, Mutex};
use std::thread;

use log::{debug, warn};
use nix::pty::openpty;
use nix::sys::termios::{LocalFlags, OutputFlags, SetArg, tcgetattr, tcsetattr};
use nix::unistd::ttyname;

use crate::State;
use crate::gdb::push_inferior_line;

/// Pty handed to gdb via `-inferior-tty-set` so the debugged process gets its
/// own terminal instead of sharing gdb's stdio (which would send its stdout
/// into the MI pipe and its stderr to /dev/null)
pub struct InferiorPty {
    master: OwnedFd,
    /// Held open so reads on `master` don't fail with EIO while no inferior is running
    _slave: OwnedFd,
    pub tty_path: String,
}

impl InferiorPty {
    pub fn open() -> Option<Self> {
        match Self::try_open() {
            Ok(pty) => Some(pty),
            Err(e) => {
                warn!("could not open pty for inferior output: {e}");
                None
            }
        }
    }

    fn try_open() -> Result<Self, nix::Error> {
        let pty = openpty(None, None)?;
        // no echo of the inferior's stdin, no \n -> \r\n translation of its output
        let mut termios = tcgetattr(&pty.slave)?;
        termios.local_flags.remove(LocalFlags::ECHO | LocalFlags::ECHONL);
        termios.output_flags.remove(OutputFlags::ONLCR);
        tcsetattr(&pty.slave, SetArg::TCSANOW, &termios)?;
        let tty_path = ttyname(&pty.slave)?.to_string_lossy().into_owned();
        Ok(Self { master: pty.master, _slave: pty.slave, tty_path })
    }
}

/// Thread reading inferior stdout/stderr from the pty master into `state.output`
pub fn spawn_reader(pty: &InferiorPty, state: Arc<Mutex<State>>) {
    let master = match pty.master.try_clone() {
        Ok(fd) => fd,
        Err(e) => {
            warn!("could not clone pty master: {e}");
            return;
        }
    };
    thread::spawn(move || {
        let mut master = File::from(master);
        let mut buf = [0u8; 4096];
        let mut pending: Vec<u8> = Vec::new();
        loop {
            match master.read(&mut buf) {
                Ok(0) => break,
                Err(e) => {
                    warn!("inferior pty read failed: {e}");
                    break;
                }
                Ok(n) => {
                    pending.extend_from_slice(&buf[..n]);
                    let mut state = state.lock().unwrap();
                    // only complete lines are shown; a partial line (no trailing
                    // newline yet) stays pending until more output arrives
                    while let Some(pos) = pending.iter().position(|&b| b == b'\n') {
                        let line: Vec<u8> = pending.drain(..=pos).collect();
                        let line = String::from_utf8_lossy(&line);
                        let line = line.trim_end_matches(['\n', '\r']);
                        debug!("inferior: {line}");
                        push_inferior_line(&mut state, line);
                    }
                }
            }
        }
    });
}
