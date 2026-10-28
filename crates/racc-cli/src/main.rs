//! racc-cli: agent a klient RemoteACC v příkazové řádce.
//!
//! Příkazy:
//! - `keygen` – nová identita zařízení
//! - `fingerprint` – otisk identity z klíče
//! - `serve` – agent: registrace na bootstrap + přímý QUIC (TTY, soubory)
//! - `tty` – vzdálený terminál přes přímé QUIC spojení
//! - `file-send` – odeslání souboru/adresáře-stylem chunků na agenta
//! - `lookup` – dotaz na bootstrap registr
//! - `relay-send` / `relay-poll` – MVP relay mailbox přes bootstrap

use std::net::{SocketAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use racc_core::crypto::Identity;
use racc_core::net::{
    check_rel_path, chunk_bytes, client_endpoint, client_endpoint_insecure, generate_self_signed,
    read_frame, reassemble, server_endpoint, write_frame, CHUNK_SIZE,
};
use racc_core::proto::{Envelope, FileChunk, Kind, TtyData};
use racc_tty::{default_shell, LocalPty, TermSize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::info;

#[derive(Debug, Parser)]
#[command(name = "racc-cli", version, about = "RemoteACC agent & client")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Vygeneruje novou identitu (hex secret do souboru).
    Keygen {
        #[arg(long)]
        out: PathBuf,
    },
    /// Vypíše fingerprint identity ze souboru s klíčem.
    Fingerprint {
        #[arg(long)]
        key: PathBuf,
    },
    /// Spustí agenta: registrace na bootstrap + přímý QUIC server.
    Serve {
        #[arg(long)]
        key: PathBuf,
        #[arg(long)]
        device_id: String,
        /// Bootstrap server `host:port`.
        #[arg(long, default_value = "127.0.0.1:8443")]
        bootstrap: String,
        /// DER certifikát bootstrap serveru (pinning). Bez něj `--insecure`.
        #[arg(long)]
        bootstrap_cert: Option<PathBuf>,
        /// Lokální bind přímého QUIC serveru.
        #[arg(long, default_value = "0.0.0.0:0")]
        bind: SocketAddr,
        /// Adresář pro příchozí soubory.
        #[arg(long, default_value = "inbox")]
        inbox: PathBuf,
        /// Uloží DER certifikát přímého serveru (pro pinning klientů).
        #[arg(long)]
        dump_cert: Option<PathBuf>,
        /// TOFU: neověřovat certifikát bootstrap serveru.
        #[arg(long)]
        insecure: bool,
        /// TLS jméno bootstrap serveru (musí souhlasit se SAN certifikátu).
        #[arg(long)]
        server_name: Option<String>,
    },
    /// Otevře vzdálený terminál na agentovi.
    Tty {
        /// Cíl `host:port` přímého QUIC serveru agenta.
        #[arg(long)]
        target: String,
        /// DER certifikát agenta (pinning). Bez něj `--insecure`.
        #[arg(long)]
        cert: Option<PathBuf>,
        #[arg(long)]
        insecure: bool,
        /// TLS jméno serveru (výchozí: host z `--target`).
        #[arg(long)]
        server_name: Option<String>,
    },
    /// Odešle soubor na agenta (chunkovaně, SHA-256, volitelně zstd).
    FileSend {
        #[arg(long)]
        target: String,
        #[arg(long)]
        file: PathBuf,
        /// Relativní cílová cesta na agentovi.
        #[arg(long)]
        dest: String,
        #[arg(long)]
        cert: Option<PathBuf>,
        #[arg(long)]
        insecure: bool,
        #[arg(long, default_value_t = true)]
        compress: bool,
        /// TLS jméno serveru (výchozí: host z `--target`).
        #[arg(long)]
        server_name: Option<String>,
    },
    /// Dotaz na registr bootstrap serveru.
    Lookup {
        #[arg(long)]
        bootstrap: String,
        #[arg(long)]
        device_id: String,
        #[arg(long)]
        cert: Option<PathBuf>,
        #[arg(long)]
        insecure: bool,
        /// TLS jméno serveru (výchozí: host z `--bootstrap`).
        #[arg(long)]
        server_name: Option<String>,
    },
    /// Uloží E2EE obálku do relay mailboxu (base64).
    RelaySend {
        #[arg(long)]
        bootstrap: String,
        #[arg(long)]
        to: String,
        #[arg(long)]
        payload_b64: String,
        #[arg(long)]
        cert: Option<PathBuf>,
        #[arg(long)]
        insecure: bool,
        /// TLS jméno serveru (výchozí: host z `--bootstrap`).
        #[arg(long)]
        server_name: Option<String>,
    },
    /// Vybere E2EE obálky z relay mailboxu.
    RelayPoll {
        #[arg(long)]
        bootstrap: String,
        #[arg(long)]
        device_id: String,
        #[arg(long)]
        cert: Option<PathBuf>,
        #[arg(long)]
        insecure: bool,
        /// TLS jméno serveru (výchozí: host z `--bootstrap`).
        #[arg(long)]
        server_name: Option<String>,
    },
}

/// TLS jméno: explicitní `--server-name`, jinak host z `host:port`.
fn tls_name(addr: &str, server_name: &Option<String>) -> String {
    match server_name {
        Some(n) => n.clone(),
        None => addr.split(':').next().unwrap_or("localhost").to_string(),
    }
}

fn load_identity(path: &Path) -> Result<Identity> {
    let text = std::fs::read_to_string(path).context("read key file")?;
    let bytes = hex::decode(text.trim()).context("key must be hex")?;
    if bytes.len() != 32 {
        bail!("key must be 32 bytes (64 hex chars)");
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Ok(Identity::from_secret_bytes(&arr))
}

fn save_identity(id: &Identity, path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)
            .context("write key file")?
            .write_all(format!("{}\n", hex::encode(id.secret_bytes())).as_bytes())?;
    }
    #[cfg(not(unix))]
    {
        std::fs::write(path, format!("{}\n", hex::encode(id.secret_bytes())))
            .context("write key file")?;
    }
    Ok(())
}

fn make_client(cert: &Option<PathBuf>, insecure: bool) -> Result<quinn::Endpoint> {
    let bind: SocketAddr = "0.0.0.0:0".parse().unwrap();
    match (cert, insecure) {
        (Some(p), _) => {
            let der = std::fs::read(p).context("read cert file")?;
            client_endpoint(bind, &der).map_err(|e| anyhow::anyhow!("{e}"))
        }
        (None, true) => client_endpoint_insecure(bind).map_err(|e| anyhow::anyhow!("{e}")),
        (None, false) => bail!("specify --cert <bootstrap.der> or --insecure (TOFU)"),
    }
}

/// Jeden požadavek/odpověď na bootstrap server (JSON rámec).
async fn bootstrap_rpc(
    bootstrap: &str,
    server_name: &Option<String>,
    cert: &Option<PathBuf>,
    insecure: bool,
    req: serde_json::Value,
) -> Result<serde_json::Value> {
    let endpoint = make_client(cert, insecure)?;
    let addr: SocketAddr = bootstrap
        .to_socket_addrs()
        .context("resolve bootstrap")?
        .next()
        .context("bootstrap has no address")?;
    let name = tls_name(bootstrap, server_name);
    let conn = endpoint.connect(addr, &name)?.await?;
    let (mut send, mut recv) = conn.open_bi().await?;
    write_frame(&mut send, &serde_json::to_vec(&req)?)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    send.finish()?;
    let resp = read_frame(&mut recv)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    conn.close(0u32.into(), b"done");
    Ok(serde_json::from_slice(&resp)?)
}

/// Parametry příkazu `serve` (struktura místo dlouhého seznamu argumentů).
struct ServeArgs {
    key: PathBuf,
    device_id: String,
    bootstrap: String,
    bootstrap_cert: Option<PathBuf>,
    bind: SocketAddr,
    inbox: PathBuf,
    dump_cert: Option<PathBuf>,
    insecure: bool,
    server_name: Option<String>,
}

async fn cmd_serve(a: ServeArgs) -> Result<()> {
    let id = load_identity(&a.key)?;
    let fingerprint = id.fingerprint();
    info!("device {} fingerprint {fingerprint}", a.device_id);
    std::fs::create_dir_all(&a.inbox)?;

    // Přímý QUIC server pro TTY + soubory.
    let (cert_der, key_der) =
        generate_self_signed(&a.device_id).map_err(|e| anyhow::anyhow!("{e}"))?;
    if let Some(p) = &a.dump_cert {
        std::fs::write(p, &cert_der)?;
        info!("direct server cert -> {}", p.display());
    }
    info!(
        "direct server cert fingerprint: {}",
        racc_core::crypto::fingerprint_of(&cert_der)
    );
    let endpoint =
        server_endpoint(a.bind, &cert_der, &key_der).map_err(|e| anyhow::anyhow!("{e}"))?;
    info!("direct QUIC on {}", endpoint.local_addr()?);

    // Registrační smyčka na pozadí (běží detached po celou dobu agenta).
    {
        let device_id = a.device_id.clone();
        let bootstrap = a.bootstrap.clone();
        let reg_cert = a.bootstrap_cert.clone();
        let reg_name = a.server_name.clone();
        let insecure = a.insecure;
        tokio::spawn(async move {
            loop {
                let req = serde_json::json!({
                    "op": "register",
                    "device_id": device_id,
                    "fingerprint": fingerprint,
                });
                match bootstrap_rpc(&bootstrap, &reg_name, &reg_cert, insecure, req).await {
                    Ok(v) => info!("bootstrap register: {v}"),
                    Err(e) => info!("bootstrap register failed (retry in 30s): {e:#}"),
                }
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            }
        });
    }

    // Smyčka přímých spojení.
    loop {
        let incoming = match endpoint.accept().await {
            Some(i) => i,
            None => break,
        };
        let inbox = a.inbox.clone();
        tokio::spawn(async move {
            let conn = match incoming.await {
                Ok(c) => c,
                Err(e) => {
                    info!("incoming failed: {e}");
                    return;
                }
            };
            info!("direct connection from {}", conn.remote_address());
            loop {
                let (mut send, mut recv) = match conn.accept_bi().await {
                    Ok(s) => s,
                    Err(_) => break,
                };
                let header = match read_frame(&mut recv).await {
                    Ok(b) => b,
                    Err(_) => break,
                };
                let hdr: serde_json::Value = match serde_json::from_slice(&header) {
                    Ok(v) => v,
                    Err(_) => break,
                };
                match hdr.get("service").and_then(|s| s.as_str()) {
                    Some("tty") => {
                        let cols = hdr.get("cols").and_then(|v| v.as_u64()).unwrap_or(80) as u32;
                        let rows = hdr.get("rows").and_then(|v| v.as_u64()).unwrap_or(24) as u32;
                        if let Err(e) = serve_tty(send, recv, cols, rows).await {
                            info!("tty session ended: {e:#}");
                        }
                    }
                    Some("file") => {
                        let resp = serve_file(recv, &inbox).await;
                        let out = match &resp {
                            Ok(path) => serde_json::json!({"ok": true, "path": path}),
                            Err(e) => serde_json::json!({"ok": false, "error": format!("{e:#}")}),
                        };
                        let bytes = serde_json::to_vec(&out).unwrap_or_default();
                        if write_frame(&mut send, &bytes).await.is_err() {
                            break;
                        }
                        let _ = send.finish();
                    }
                    other => {
                        info!("unknown service: {other:?}");
                        break;
                    }
                }
            }
        });
    }
    Ok(())
}

/// Serverová strana TTY: PTY <-> QUIC obálky `TtyData`.
async fn serve_tty(
    mut send: quinn::SendStream,
    mut recv: quinn::RecvStream,
    cols: u32,
    rows: u32,
) -> Result<()> {
    let (mut pty, mut events) = LocalPty::spawn(default_shell(), TermSize { cols, rows })
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let session = b"tty-session-000001";
    loop {
        tokio::select! {
            ev = events.recv() => match ev {
                Some(racc_tty::PtyEvent::Data(d)) => {
                    let env = Envelope::wrap(Kind::Tty, session, &TtyData {
                        pty_id: 0, data: d, resize: None, eof: false,
                    });
                    if write_frame(&mut send, &env.encode_vec()).await.is_err() { break; }
                }
                _ => {
                    let env = Envelope::wrap(Kind::Tty, session, &TtyData {
                        pty_id: 0, data: Vec::new(), resize: None, eof: true,
                    });
                    let _ = write_frame(&mut send, &env.encode_vec()).await;
                    break;
                }
            },
            frame = read_frame(&mut recv) => {
                let frame = match frame {
                    Ok(f) => f,
                    Err(_) => break,
                };
                let env = match Envelope::decode_vec(&frame) {
                    Ok(e) => e,
                    Err(_) => break,
                };
                let msg: TtyData = match env.unwrap(Kind::Tty) {
                    Ok(m) => m,
                    Err(_) => break,
                };
                if let Some(r) = msg.resize {
                    let _ = pty.resize(TermSize { cols: r.cols, rows: r.rows });
                }
                if !msg.data.is_empty() && pty.write(&msg.data).is_err() {
                    break;
                }
                if msg.eof { break; }
            }
        }
    }
    let _ = send.finish();
    Ok(())
}

/// Serverová strana souborů: načte chunky do `finish()`, ověří, zapíše.
async fn serve_file(mut recv: quinn::RecvStream, inbox: &Path) -> Result<String> {
    let mut chunks: Vec<FileChunk> = Vec::new();
    // Čteme do `finish()` klienta (ukončení zapisovací strany).
    while let Ok(f) = read_frame(&mut recv).await {
        let env = Envelope::decode_vec(&f).map_err(|e| anyhow::anyhow!("{e}"))?;
        let c: FileChunk = env.unwrap(Kind::File).map_err(|e| anyhow::anyhow!("{e}"))?;
        chunks.push(c);
    }
    if chunks.is_empty() {
        bail!("no chunks received");
    }
    let rel = chunks[0].path.clone();
    check_rel_path(&rel).map_err(|e| anyhow::anyhow!("{e}"))?;
    let data = reassemble(&chunks).map_err(|e| anyhow::anyhow!("{e}"))?;
    let dest = inbox.join(&rel);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&dest, data)?;
    info!("file stored: {} ({} chunks)", dest.display(), chunks.len());
    Ok(rel)
}

async fn cmd_tty(
    target: &str,
    cert: &Option<PathBuf>,
    insecure: bool,
    server_name: &Option<String>,
) -> Result<()> {
    let endpoint = make_client(cert, insecure)?;
    let addr: SocketAddr = target
        .to_socket_addrs()
        .context("resolve target")?
        .next()
        .context("no addr")?;
    let name = tls_name(target, server_name);
    let conn = endpoint.connect(addr, &name)?.await?;
    let (mut send, mut recv) = conn.open_bi().await?;
    write_frame(&mut send, br#"{"service":"tty","cols":120,"rows":30}"#)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let session = b"tty-session-000001";
    let mut stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut in_buf = [0u8; 4096];
    loop {
        tokio::select! {
            n = stdin.read(&mut in_buf) => {
                let n = n?;
                if n == 0 {
                    let env = Envelope::wrap(Kind::Tty, session, &TtyData {
                        pty_id: 0, data: Vec::new(), resize: None, eof: true,
                    });
                    write_frame(&mut send, &env.encode_vec()).await.map_err(|e| anyhow::anyhow!("{e}"))?;
                    break;
                }
                let env = Envelope::wrap(Kind::Tty, session, &TtyData {
                    pty_id: 0, data: in_buf[..n].to_vec(), resize: None, eof: false,
                });
                write_frame(&mut send, &env.encode_vec()).await.map_err(|e| anyhow::anyhow!("{e}"))?;
            }
            frame = read_frame(&mut recv) => {
                let frame = match frame {
                    Ok(f) => f,
                    Err(_) => break,
                };
                let env = Envelope::decode_vec(&frame).map_err(|e| anyhow::anyhow!("{e}"))?;
                let msg: TtyData = env.unwrap(Kind::Tty).map_err(|e| anyhow::anyhow!("{e}"))?;
                if !msg.data.is_empty() {
                    stdout.write_all(&msg.data).await?;
                    stdout.flush().await?;
                }
                if msg.eof { break; }
            }
        }
    }
    let _ = send.finish();
    conn.close(0u32.into(), b"done");
    Ok(())
}

async fn cmd_file_send(
    target: &str,
    file: &Path,
    dest: &str,
    cert: &Option<PathBuf>,
    insecure: bool,
    compress: bool,
    server_name: &Option<String>,
) -> Result<()> {
    check_rel_path(dest).map_err(|e| anyhow::anyhow!("{e}"))?;
    let data = std::fs::read(file).context("read file")?;
    let endpoint = make_client(cert, insecure)?;
    let addr: SocketAddr = target
        .to_socket_addrs()
        .context("resolve target")?
        .next()
        .context("no addr")?;
    let name = tls_name(target, server_name);
    let conn = endpoint.connect(addr, &name)?.await?;
    let (mut send, mut recv) = conn.open_bi().await?;
    write_frame(&mut send, br#"{"service":"file"}"#)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let transfer_id = format!("tx-{}", &racc_core::crypto::fingerprint_of(&data)[..16]);
    let session = b"file-session-000001";
    let chunks = chunk_bytes(&transfer_id, dest, &data, CHUNK_SIZE, compress);
    for c in &chunks {
        let env = Envelope::wrap(Kind::File, session, c);
        write_frame(&mut send, &env.encode_vec())
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
    }
    send.finish()?;
    let resp = read_frame(&mut recv)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    conn.close(0u32.into(), b"done");
    let v: serde_json::Value = serde_json::from_slice(&resp)?;
    println!("{v}");
    if v.get("ok").and_then(|x| x.as_bool()) != Some(true) {
        bail!("server rejected transfer: {v}");
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("racc_cli=info".parse()?),
        )
        .init();
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Keygen { out } => {
            let id = Identity::generate();
            save_identity(&id, &out)?;
            println!("fingerprint: {}", id.fingerprint());
            println!("key saved: {}", out.display());
        }
        Cmd::Fingerprint { key } => {
            let id = load_identity(&key)?;
            println!("{}", id.fingerprint());
        }
        Cmd::Serve {
            key,
            device_id,
            bootstrap,
            bootstrap_cert,
            bind,
            inbox,
            dump_cert,
            insecure,
            server_name,
        } => {
            cmd_serve(ServeArgs {
                key,
                device_id,
                bootstrap,
                bootstrap_cert,
                bind,
                inbox,
                dump_cert,
                insecure,
                server_name,
            })
            .await?;
        }
        Cmd::Tty {
            target,
            cert,
            insecure,
            server_name,
        } => cmd_tty(&target, &cert, insecure, &server_name).await?,
        Cmd::FileSend {
            target,
            file,
            dest,
            cert,
            insecure,
            compress,
            server_name,
        } => {
            cmd_file_send(
                &target,
                &file,
                &dest,
                &cert,
                insecure,
                compress,
                &server_name,
            )
            .await?
        }
        Cmd::Lookup {
            bootstrap,
            device_id,
            cert,
            insecure,
            server_name,
        } => {
            let v = bootstrap_rpc(
                &bootstrap,
                &server_name,
                &cert,
                insecure,
                serde_json::json!({"op": "lookup", "device_id": device_id}),
            )
            .await?;
            println!("{v}");
        }
        Cmd::RelaySend {
            bootstrap,
            to,
            payload_b64,
            cert,
            insecure,
            server_name,
        } => {
            let v = bootstrap_rpc(
                &bootstrap,
                &server_name,
                &cert,
                insecure,
                serde_json::json!({"op": "send", "to": to, "payload_b64": payload_b64}),
            )
            .await?;
            println!("{v}");
        }
        Cmd::RelayPoll {
            bootstrap,
            device_id,
            cert,
            insecure,
            server_name,
        } => {
            let v = bootstrap_rpc(
                &bootstrap,
                &server_name,
                &cert,
                insecure,
                serde_json::json!({"op": "poll", "device_id": device_id}),
            )
            .await?;
            println!("{v}");
        }
    }
    Ok(())
}
