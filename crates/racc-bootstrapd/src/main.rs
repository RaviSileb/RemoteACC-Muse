//! racc-bootstrapd: signalizační server RemoteACC pro VPS.
//!
//! QUIC server (TCP+UDP fallback řeší QUIC nad UDP; relay dat je MVP mailbox).
//! Operace (JSON rámce, viz `docs/PROTOKOL.md`):
//! register / lookup / list / send / poll / health.
//! Server vidí jen šifrované obálky + směrovací metadata (E2EE zero-trust).

use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use racc_core::net::{generate_self_signed, read_frame, server_endpoint, write_frame};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::{info, warn};

#[derive(Debug, Parser)]
#[command(
    name = "racc-bootstrapd",
    version,
    about = "RemoteACC signaling server"
)]
struct Args {
    /// Adresa pro poslech (QUIC/UDP).
    #[arg(long, default_value = "0.0.0.0:8443")]
    bind: SocketAddr,
    /// Soubor s TLS certifikátem (DER). Bez něj se vygeneruje ephemeral.
    #[arg(long)]
    cert: Option<PathBuf>,
    /// Soubor s TLS klíčem (PKCS#8 DER).
    #[arg(long)]
    key: Option<PathBuf>,
    /// Kam uložit vygenerovaný certifikát (pro pinning klientů).
    #[arg(long)]
    dump_cert: Option<PathBuf>,
}

/// Záznam zařízení v registru.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeviceInfo {
    device_id: String,
    fingerprint: String,
    /// Veřejně viditelná adresa (z QUIC spojení).
    addr: String,
    /// Volitelná E2EE obálka s kandidáty (server ji nečte).
    blob_b64: Option<String>,
    last_seen_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Request {
    op: String,
    #[serde(default)]
    device_id: Option<String>,
    #[serde(default)]
    fingerprint: Option<String>,
    #[serde(default)]
    blob_b64: Option<String>,
    #[serde(default)]
    to: Option<String>,
    #[serde(default)]
    payload_b64: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Response {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    device: Option<DeviceInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    devices: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    messages: Option<Vec<String>>,
}

impl Response {
    fn ok() -> Self {
        Self {
            ok: true,
            error: None,
            device: None,
            devices: None,
            messages: None,
        }
    }
    fn err(msg: impl Into<String>) -> Self {
        Self {
            ok: false,
            error: Some(msg.into()),
            device: None,
            devices: None,
            messages: None,
        }
    }
}

struct State {
    devices: HashMap<String, DeviceInfo>,
    /// MVP relay mailbox: fronta E2EE obálek pro offline/nedostupná zařízení.
    mailbox: HashMap<String, VecDeque<String>>,
}

impl State {
    fn new() -> Self {
        Self {
            devices: HashMap::new(),
            mailbox: HashMap::new(),
        }
    }

    fn handle(&mut self, req: Request, peer: SocketAddr) -> Response {
        match req.op.as_str() {
            "health" => Response::ok(),
            "register" => {
                let Some(device_id) = req.device_id else {
                    return Response::err("register: missing device_id");
                };
                if device_id.is_empty() || device_id.len() > 128 {
                    return Response::err("register: bad device_id");
                }
                let info = DeviceInfo {
                    device_id: device_id.clone(),
                    fingerprint: req.fingerprint.unwrap_or_default(),
                    addr: peer.to_string(),
                    blob_b64: req.blob_b64,
                    last_seen_unix: now_unix(),
                };
                self.devices.insert(device_id, info);
                Response::ok()
            }
            "lookup" => {
                let Some(device_id) = req.device_id else {
                    return Response::err("lookup: missing device_id");
                };
                match self.devices.get(&device_id) {
                    Some(d) => Response {
                        ok: true,
                        error: None,
                        device: Some(d.clone()),
                        devices: None,
                        messages: None,
                    },
                    None => Response::err("lookup: unknown device"),
                }
            }
            "list" => {
                let mut ids: Vec<String> = self.devices.keys().cloned().collect();
                ids.sort();
                Response {
                    ok: true,
                    error: None,
                    device: None,
                    devices: Some(ids),
                    messages: None,
                }
            }
            "send" => {
                let (Some(to), Some(payload)) = (req.to, req.payload_b64) else {
                    return Response::err("send: missing to/payload_b64");
                };
                if payload.len() > 1024 * 1024 {
                    return Response::err("send: payload too large (max 1 MiB)");
                }
                let q = self.mailbox.entry(to).or_default();
                if q.len() >= 100 {
                    return Response::err("send: mailbox full");
                }
                q.push_back(payload);
                Response::ok()
            }
            "poll" => {
                let Some(device_id) = req.device_id else {
                    return Response::err("poll: missing device_id");
                };
                let msgs: Vec<String> = self
                    .mailbox
                    .remove(&device_id)
                    .map(|q| q.into())
                    .unwrap_or_default();
                Response {
                    ok: true,
                    error: None,
                    device: None,
                    devices: None,
                    messages: Some(msgs),
                }
            }
            other => Response::err(format!("unknown op '{other}'")),
        }
    }
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("racc_bootstrapd=info".parse()?),
        )
        .init();
    let args = Args::parse();

    let (cert_der, key_der) = match (args.cert, args.key) {
        (Some(c), Some(k)) => (std::fs::read(c)?, std::fs::read(k)?),
        (None, None) => {
            warn!("no --cert/--key: generating ephemeral self-signed certificate");
            generate_self_signed("remoteacc-bootstrap").map_err(|e| anyhow::anyhow!("{e}"))?
        }
        _ => anyhow::bail!("specify both --cert and --key, or neither"),
    };
    if let Some(path) = args.dump_cert {
        std::fs::write(&path, &cert_der)?;
        info!("wrote certificate to {}", path.display());
    }
    info!(
        "bootstrap fingerprint (sha256): {}",
        racc_core::crypto::fingerprint_of(&cert_der)
    );

    let endpoint =
        server_endpoint(args.bind, &cert_der, &key_der).map_err(|e| anyhow::anyhow!("{e}"))?;
    info!("listening on {} (QUIC)", endpoint.local_addr()?);
    let state = Arc::new(Mutex::new(State::new()));

    loop {
        let incoming = match endpoint.accept().await {
            Some(i) => i,
            None => break,
        };
        let state = state.clone();
        tokio::spawn(async move {
            let conn = match incoming.await {
                Ok(c) => c,
                Err(e) => {
                    warn!("incoming failed: {e}");
                    return;
                }
            };
            let peer = conn.remote_address();
            loop {
                let (mut send, mut recv) = match conn.accept_bi().await {
                    Ok(s) => s,
                    Err(_) => break,
                };
                let req_bytes = match read_frame(&mut recv).await {
                    Ok(b) => b,
                    Err(e) => {
                        warn!("read from {peer}: {e}");
                        break;
                    }
                };
                let resp = match serde_json::from_slice::<Request>(&req_bytes) {
                    Ok(req) => state.lock().await.handle(req, peer),
                    Err(e) => Response::err(format!("bad json: {e}")),
                };
                let out = serde_json::to_vec(&resp).unwrap_or_default();
                if write_frame(&mut send, &out).await.is_err() {
                    break;
                }
                if send.finish().is_err() {
                    break;
                }
            }
        });
    }
    Ok(())
}

// Speciální pomocná funkce, aby binárku šlo použít z integračních testů.
#[allow(dead_code)]
fn _ensure_send_sync() {
    fn assert<T: Send + Sync>() {}
    assert::<State>();
}
