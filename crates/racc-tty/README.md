# racc-tty

PTY subsystém RemoteACC. `LocalPty::spawn` spouští shell v pseudoterminálu
(ConPTY na Windows, `forkpty` na unixu přes `portable-pty`), I/O proudí
přes `tokio::sync::mpsc` kanál událostí `PtyEvent::{Data, Exit}`.

Napojení na datovou rovinu: `TtyData` obálky (`racc-core`), viz `racc-cli`.
