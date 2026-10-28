//! E2E: spustí skutečný `racc-bootstrapd` a ověří register/lookup/send/poll.

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use racc_core::net::{client_endpoint, read_frame, write_frame};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

/// Cesta k binárce vedle `target/debug/deps/<test>`.
fn bin_path(name: &str) -> PathBuf {
    let exe = std::env::current_exe().expect("current_exe");
    let debug = exe.parent().and_then(|p| p.parent()).expect("target dir");
    let file = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    debug.join(file)
}

struct Server {
    child: Child,
    addr: String,
}

impl Server {
    async fn spawn(cert_out: &std::path::Path) -> Self {
        let mut child = Command::new(bin_path("racc-bootstrapd"))
            .args([
                "--bind",
                "127.0.0.1:0",
                "--dump-cert",
                cert_out.to_str().unwrap(),
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn racc-bootstrapd");
        let stderr = child.stderr.take().expect("stderr");
        let mut lines = BufReader::new(stderr).lines();
        let addr = tokio::time::timeout(Duration::from_secs(15), async {
            while let Some(line) = lines.next_line().await.expect("read line") {
                if let Some(rest) = line.split("listening on ").nth(1) {
                    return rest.split_whitespace().next().unwrap().to_string();
                }
            }
            panic!("server printed no listening line (EOF)");
        })
        .await
        .expect("server start timeout");
        Self { child, addr }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

async fn rpc(addr: &str, cert_der: &[u8], req: serde_json::Value) -> serde_json::Value {
    let endpoint =
        client_endpoint("127.0.0.1:0".parse().unwrap(), cert_der).expect("client endpoint");
    let sock: std::net::SocketAddr = addr.parse().unwrap();
    // Ephemeral bootstrap cert má SAN "remoteacc-bootstrap".
    let conn = endpoint
        .connect(sock, "remoteacc-bootstrap")
        .expect("connect")
        .await
        .expect("handshake");
    let (mut send, mut recv) = conn.open_bi().await.expect("open_bi");
    write_frame(&mut send, &serde_json::to_vec(&req).unwrap())
        .await
        .expect("write");
    send.finish().expect("finish");
    let resp = read_frame(&mut recv).await.expect("read");
    conn.close(0u32.into(), b"done");
    serde_json::from_slice(&resp).unwrap()
}

#[tokio::test]
async fn bootstrap_register_lookup_relay() {
    let tmp = tempfile::tempdir().unwrap();
    let cert_path = tmp.path().join("boot.der");
    let server = Server::spawn(&cert_path).await;
    // dump-cert se zapisuje před listen; pro jistotu počkáme na soubor.
    tokio::time::timeout(Duration::from_secs(5), async {
        while !cert_path.exists() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    let cert_der = std::fs::read(&cert_path).unwrap();

    let reg = rpc(
        &server.addr,
        &cert_der,
        serde_json::json!({
            "op": "register",
            "device_id": "dev-e2e",
            "fingerprint": "aa11",
        }),
    )
    .await;
    assert_eq!(reg["ok"], true);

    let found = rpc(
        &server.addr,
        &cert_der,
        serde_json::json!({"op": "lookup", "device_id": "dev-e2e"}),
    )
    .await;
    assert_eq!(found["ok"], true);
    assert_eq!(found["device"]["fingerprint"], "aa11");

    let missing = rpc(
        &server.addr,
        &cert_der,
        serde_json::json!({"op": "lookup", "device_id": "nope"}),
    )
    .await;
    assert_eq!(missing["ok"], false);

    let sent = rpc(
        &server.addr,
        &cert_der,
        serde_json::json!({"op": "send", "to": "dev-e2e", "payload_b64": "aGVsbG8="}),
    )
    .await;
    assert_eq!(sent["ok"], true);

    let polled = rpc(
        &server.addr,
        &cert_der,
        serde_json::json!({"op": "poll", "device_id": "dev-e2e"}),
    )
    .await;
    assert_eq!(polled["messages"], serde_json::json!(["aGVsbG8="]));

    // Druhý poll je prázdný (doručeno právě jednou).
    let polled2 = rpc(
        &server.addr,
        &cert_der,
        serde_json::json!({"op": "poll", "device_id": "dev-e2e"}),
    )
    .await;
    assert_eq!(polled2["messages"], serde_json::json!([]));
}
