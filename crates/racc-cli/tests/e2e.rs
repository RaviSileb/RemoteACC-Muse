//! E2E: keygen + serve + file-send přes skutečné binárky `racc-cli`.

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

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

async fn run(args: &[&str]) -> (bool, String) {
    let out = Command::new(bin_path("racc-cli"))
        .args(args)
        .output()
        .await
        .expect("spawn racc-cli");
    let mut text = String::from_utf8_lossy(&out.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.success(), text)
}

struct Agent {
    child: Child,
    addr: String,
    log: std::sync::Arc<tokio::sync::Mutex<Vec<String>>>,
}

impl Agent {
    async fn spawn(tmp: &std::path::Path, key: &std::path::Path) -> Self {
        let cert = tmp.join("direct.der");
        let inbox = tmp.join("inbox");
        let mut child = Command::new(bin_path("racc-cli"))
            .args([
                "serve",
                "--key",
                key.to_str().unwrap(),
                "--device-id",
                "e2e-agent",
                "--bootstrap",
                "127.0.0.1:9",
                "--insecure",
                "--bind",
                "127.0.0.1:0",
                "--inbox",
                inbox.to_str().unwrap(),
                "--dump-cert",
                cert.to_str().unwrap(),
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn serve");
        let stderr = child.stderr.take().expect("stderr");
        let mut lines = BufReader::new(stderr).lines();
        let log = std::sync::Arc::new(tokio::sync::Mutex::new(Vec::<String>::new()));
        let addr = tokio::time::timeout(Duration::from_secs(20), async {
            while let Some(line) = lines.next_line().await.expect("read line") {
                log.lock().await.push(line.clone());
                if let Some(rest) = line.split("direct QUIC on ").nth(1) {
                    return rest.split_whitespace().next().unwrap().to_string();
                }
            }
            panic!("serve printed no direct-QUIC line");
        })
        .await
        .expect("serve start timeout");
        // Pokračuj ve čtení logu na pozadí pro diagnostiku.
        let log_bg = log.clone();
        tokio::spawn(async move {
            while let Ok(Some(line)) = lines.next_line().await {
                log_bg.lock().await.push(line);
            }
        });
        Self { child, addr, log }
    }
}

impl Drop for Agent {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

#[tokio::test]
async fn file_send_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    let key = tmp.path().join("device.key");

    let (ok, out) = run(&["keygen", "--out", key.to_str().unwrap()]).await;
    assert!(ok, "keygen failed: {out}");
    assert!(out.contains("fingerprint: "));

    let (ok, out) = run(&["fingerprint", "--key", key.to_str().unwrap()]).await;
    assert!(ok, "fingerprint failed: {out}");

    let agent = Agent::spawn(tmp.path(), &key).await;

    // Větší než 1 chunk, aby se ověřilo skládání (chunk = 256 KiB).
    let payload: Vec<u8> = (0..700_000u32).map(|i| (i % 251) as u8).collect();
    let src = tmp.path().join("input.bin");
    std::fs::write(&src, &payload).unwrap();

    let cert = tmp.path().join("direct.der");
    // dump-cert vzniká před listen; pro jistotu počkat.
    tokio::time::timeout(Duration::from_secs(5), async {
        while !cert.exists() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();

    let (ok, out) = run(&[
        "file-send",
        "--target",
        &agent.addr,
        "--cert",
        cert.to_str().unwrap(),
        "--server-name",
        "e2e-agent",
        "--file",
        src.to_str().unwrap(),
        "--dest",
        "sub/out.bin",
    ])
    .await;
    if !ok {
        eprintln!("AGENT ADDR: {}", agent.addr);
        for line in agent.log.lock().await.iter() {
            eprintln!("AGENT: {line}");
        }
    }
    assert!(ok, "file-send failed: {out}");
    assert!(out.contains("\"ok\":true"), "unexpected reply: {out}");

    let stored = tmp.path().join("inbox").join("sub").join("out.bin");
    // Krátké čekání na flush serveru.
    tokio::time::timeout(Duration::from_secs(5), async {
        while std::fs::read(&stored).map(|d| d.len()).unwrap_or(0) != payload.len() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("file not stored");
    assert_eq!(std::fs::read(&stored).unwrap(), payload);

    // Path traversal musí server odmítnout.
    let (ok, out) = run(&[
        "file-send",
        "--target",
        &agent.addr,
        "--cert",
        cert.to_str().unwrap(),
        "--server-name",
        "e2e-agent",
        "--file",
        src.to_str().unwrap(),
        "--dest",
        "../evil.bin",
    ])
    .await;
    assert!(!ok, "traversal unexpectedly accepted: {out}");
}
