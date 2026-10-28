//! racc-tty: PTY subsystém RemoteACC.
//!
//! OS vrstva: ConPTY (Windows), `openpty`/`forkpty` (Linux/macOS) přes
//! `portable-pty`, NDK pty (Android), iOS pouze klient (bez serverové PTY).
//! Propojení na QUIC datovou rovinu zajišťuje [`PtySession`] + `TtyData` zprávy.

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use thiserror::Error;
use tokio::sync::mpsc;

#[derive(Debug, Error)]
pub enum TtyError {
    #[error("pty: {0}")]
    Pty(String),
    #[error("io: {0}")]
    Io(String),
    #[error("session closed")]
    Closed,
}

pub type Result<T> = std::result::Result<T, TtyError>;

/// Velikost terminálu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TermSize {
    pub cols: u32,
    pub rows: u32,
}

impl Default for TermSize {
    fn default() -> Self {
        Self { cols: 80, rows: 24 }
    }
}

/// Událost z PTY.
#[derive(Debug)]
pub enum PtyEvent {
    Data(Vec<u8>),
    Exit(i32),
}

/// Běžící lokální PTY sezení. Čtení probíhá na blokujícím vlákně,
/// události chodí přes kanál (neblokuje tokio runtime).
pub struct LocalPty {
    child: Box<dyn Child + Send>,
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
}

impl LocalPty {
    /// Spustí příkaz v novém PTY. Vrací sezení a kanál událostí.
    pub fn spawn(shell: &str, size: TermSize) -> Result<(Self, mpsc::Receiver<PtyEvent>)> {
        let pty = native_pty_system();
        let pair = pty
            .openpty(PtySize {
                rows: size.rows as u16,
                cols: size.cols as u16,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| TtyError::Pty(e.to_string()))?;
        let mut cmd = CommandBuilder::new(shell);
        // Interaktivní shell bez konfiguračních zkratek, ať je chování stabilní.
        cmd.env("TERM", "xterm-256color");
        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| TtyError::Pty(e.to_string()))?;
        drop(pair.slave);
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| TtyError::Pty(e.to_string()))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| TtyError::Pty(e.to_string()))?;
        let (tx, rx) = mpsc::channel::<PtyEvent>(64);
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        let _ = tx.blocking_send(PtyEvent::Exit(0));
                        break;
                    }
                    Ok(n) => {
                        if tx.blocking_send(PtyEvent::Data(buf[..n].to_vec())).is_err() {
                            break;
                        }
                    }
                    Err(_) => {
                        let _ = tx.blocking_send(PtyEvent::Exit(1));
                        break;
                    }
                }
            }
        });
        Ok((
            Self {
                child,
                writer,
                master: pair.master,
            },
            rx,
        ))
    }

    /// Zapíše vstup do PTY.
    pub fn write(&mut self, data: &[u8]) -> Result<()> {
        self.writer
            .write_all(data)
            .map_err(|e| TtyError::Io(e.to_string()))?;
        self.writer
            .flush()
            .map_err(|e| TtyError::Io(e.to_string()))?;
        Ok(())
    }

    /// Změní velikost PTY.
    pub fn resize(&self, size: TermSize) -> Result<()> {
        self.master
            .resize(PtySize {
                rows: size.rows as u16,
                cols: size.cols as u16,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| TtyError::Pty(e.to_string()))
    }

    /// Ukončí proces v PTY.
    pub fn kill(mut self) -> Result<()> {
        self.child.kill().map_err(|e| TtyError::Pty(e.to_string()))
    }
}

/// Výchozí shell pro danou platformu.
pub fn default_shell() -> &'static str {
    if cfg!(windows) {
        "powershell.exe"
    } else {
        "sh"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_term_size() {
        assert_eq!(TermSize::default().cols, 80);
    }

    #[tokio::test]
    async fn spawn_echo_and_read() {
        let shell = default_shell();
        let (mut pty, mut rx) = LocalPty::spawn(shell, TermSize::default()).unwrap();
        pty.write(b"echo remoteacc-pty-ok\n").unwrap();
        let deadline = tokio::time::sleep(std::time::Duration::from_secs(10));
        tokio::pin!(deadline);
        let mut collected = Vec::new();
        let found = loop {
            tokio::select! {
                ev = rx.recv() => match ev {
                    Some(PtyEvent::Data(d)) => {
                        collected.extend_from_slice(&d);
                        if collected.windows(b"remoteacc-pty-ok".len()).any(|w| w == b"remoteacc-pty-ok") {
                            break true;
                        }
                    }
                    _ => break false,
                },
                _ = &mut deadline => break false,
            }
        };
        pty.kill().ok();
        assert!(found, "pty did not echo marker");
    }
}
