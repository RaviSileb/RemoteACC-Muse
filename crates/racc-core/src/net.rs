//! QUIC transport (quinn), rámování streamů a chunkovaný přenos souborů.

use std::net::SocketAddr;
use std::sync::Arc;

use quinn::{ClientConfig, Endpoint, RecvStream, SendStream, ServerConfig};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};
use crate::proto::FileChunk;

/// Maximální velikost jednoho rámce (16 MiB).
pub const MAX_FRAME: usize = 16 * 1024 * 1024;
/// Výchozí velikost chunku souboru (256 KiB).
pub const CHUNK_SIZE: usize = 256 * 1024;

// ---------------------------------------------------------------------------
// Certifikáty a endpointy
// ---------------------------------------------------------------------------

/// Vygeneruje self-signed certifikát pro bootstrap server / testy.
/// Vrací (cert_der, pkcs8_key_der).
pub fn generate_self_signed(subject: &str) -> Result<(Vec<u8>, Vec<u8>)> {
    let key = rcgen::generate_simple_self_signed(vec![subject.to_string()])
        .map_err(|e| Error::Transport(e.to_string()))?;
    let cert_der = key.cert.der().to_vec();
    let key_der = key.key_pair.serialize_der();
    Ok((cert_der, key_der))
}

/// QUIC server endpoint na dané adrese.
pub fn server_endpoint(addr: SocketAddr, cert_der: &[u8], key_der: &[u8]) -> Result<Endpoint> {
    let cert = CertificateDer::from(cert_der.to_vec());
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key_der.to_vec()));
    let server_cfg = ServerConfig::with_single_cert(vec![cert], key)
        .map_err(|e| Error::Transport(format!("server tls config: {e}")))?;
    Endpoint::server(server_cfg, addr).map_err(|e| Error::Transport(format!("bind {addr}: {e}")))
}

/// Nainstaluje výchozího poskytovatele šifer (ring), pokud ještě není.
fn ensure_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

/// QUIC klient endpoint. `server_cert_der` je očekávaný certifikát serveru
/// (pinning); prázdné = nedůvěřovat ničemu (testy musí předat cert).
pub fn client_endpoint(bind: SocketAddr, server_cert_der: &[u8]) -> Result<Endpoint> {
    ensure_crypto_provider();
    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(CertificateDer::from(server_cert_der.to_vec()))
        .map_err(|e| Error::Transport(format!("bad server cert: {e}")))?;
    let tls = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let quic_tls = quinn::crypto::rustls::QuicClientConfig::try_from(tls)
        .map_err(|e| Error::Transport(format!("client tls config: {e}")))?;
    let mut endpoint =
        Endpoint::client(bind).map_err(|e| Error::Transport(format!("bind {bind}: {e}")))?;
    endpoint.set_default_client_config(ClientConfig::new(Arc::new(quic_tls)));
    Ok(endpoint)
}

/// Nebezpečný verifikátor pro TOFU režim (`--insecure`): přijme jakýkoli
/// certifikát. E2EE vrstva (handshake identit) stále chrání obsah.
#[derive(Debug)]
struct NoServerVerify;

impl rustls::client::danger::ServerCertVerifier for NoServerVerify {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> std::result::Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// QUIC klient endpoint bez ověření serverového certifikátu (TOFU).
/// Používejte jen s následným pinningem otisku!
pub fn client_endpoint_insecure(bind: SocketAddr) -> Result<Endpoint> {
    ensure_crypto_provider();
    let provider = rustls::crypto::ring::default_provider();
    let tls = rustls::ClientConfig::builder_with_provider(provider.into())
        .with_safe_default_protocol_versions()
        .map_err(|e| Error::Transport(format!("tls versions: {e}")))?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(NoServerVerify))
        .with_no_client_auth();
    let quic_tls = quinn::crypto::rustls::QuicClientConfig::try_from(tls)
        .map_err(|e| Error::Transport(format!("client tls config: {e}")))?;
    let mut endpoint =
        Endpoint::client(bind).map_err(|e| Error::Transport(format!("bind {bind}: {e}")))?;
    endpoint.set_default_client_config(ClientConfig::new(Arc::new(quic_tls)));
    Ok(endpoint)
}

// ---------------------------------------------------------------------------
// Rámování: u32 BE délka + bajty
// ---------------------------------------------------------------------------

/// Zapíše délkově prefixovaný rámec.
pub async fn write_frame(send: &mut SendStream, bytes: &[u8]) -> Result<()> {
    if bytes.len() > MAX_FRAME {
        return Err(Error::Transport(format!(
            "frame too large: {} bytes",
            bytes.len()
        )));
    }
    let len = (bytes.len() as u32).to_be_bytes();
    send.write_all(&len)
        .await
        .map_err(|e| Error::Transport(e.to_string()))?;
    send.write_all(bytes)
        .await
        .map_err(|e| Error::Transport(e.to_string()))?;
    Ok(())
}

/// Přečte jeden délkově prefixovaný rámec.
pub async fn read_frame(recv: &mut RecvStream) -> Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    recv.read_exact(&mut len_buf)
        .await
        .map_err(|e| Error::Transport(format!("frame header: {e}")))?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > MAX_FRAME {
        return Err(Error::Transport(format!("frame too large: {len} bytes")));
    }
    let mut buf = vec![0u8; len];
    recv.read_exact(&mut buf)
        .await
        .map_err(|e| Error::Transport(format!("frame body: {e}")))?;
    Ok(buf)
}

// ---------------------------------------------------------------------------
// Chunkovaný přenos souborů (SHA-256 na chunk, volitelně zstd)
// ---------------------------------------------------------------------------

/// Rozdělí data na chunky s per-chunk SHA-256.
pub fn chunk_bytes(
    transfer_id: &str,
    path: &str,
    data: &[u8],
    chunk_size: usize,
    compress: bool,
) -> Vec<FileChunk> {
    assert!(chunk_size > 0, "chunk_size must be non-zero");
    let total = data.len().div_ceil(chunk_size).max(1) as u64;
    let mut out = Vec::new();
    if data.is_empty() {
        let digest = Sha256::digest([]);
        let payload = if compress {
            zstd::stream::encode_all(std::io::empty(), 3).unwrap_or_default()
        } else {
            Vec::new()
        };
        out.push(FileChunk {
            transfer_id: transfer_id.to_string(),
            path: path.to_string(),
            index: 0,
            total_chunks: 1,
            sha256: digest.to_vec(),
            compressed: compress,
            data: payload,
            last: true,
        });
        return out;
    }
    for (i, piece) in data.chunks(chunk_size).enumerate() {
        let digest = Sha256::digest(piece);
        let payload = if compress {
            zstd::stream::encode_all(piece, 3).unwrap_or_else(|_| piece.to_vec())
        } else {
            piece.to_vec()
        };
        out.push(FileChunk {
            transfer_id: transfer_id.to_string(),
            path: path.to_string(),
            index: i as u64,
            total_chunks: total,
            sha256: digest.to_vec(),
            compressed: compress,
            data: payload,
            last: i as u64 + 1 == total,
        });
    }
    out
}

/// Ověří chunky (pořadí, SHA-256, dekomprese) a složí původní data.
pub fn reassemble(chunks: &[FileChunk]) -> Result<Vec<u8>> {
    if chunks.is_empty() {
        return Err(Error::FileTransfer("no chunks".into()));
    }
    let total = chunks[0].total_chunks;
    if chunks.len() as u64 != total {
        return Err(Error::FileTransfer(format!(
            "expected {total} chunks, got {}",
            chunks.len()
        )));
    }
    let mut out = Vec::new();
    for (i, c) in chunks.iter().enumerate() {
        if c.index != i as u64 {
            return Err(Error::FileTransfer(format!(
                "chunk out of order: expected {i}, got {}",
                c.index
            )));
        }
        let raw = if c.compressed {
            zstd::stream::decode_all(&*c.data)
                .map_err(|e| Error::FileTransfer(format!("zstd: {e}")))?
        } else {
            c.data.clone()
        };
        let digest = Sha256::digest(&raw);
        if digest.as_slice() != c.sha256.as_slice() {
            return Err(Error::FileTransfer(format!("chunk {i} sha256 mismatch")));
        }
        out.extend_from_slice(&raw);
    }
    if !chunks.last().map(|c| c.last).unwrap_or(false) {
        return Err(Error::FileTransfer("missing last flag".into()));
    }
    Ok(out)
}

/// Odmítne nebezpečné cílové cesty (`..`, absolutní cesty).
pub fn check_rel_path(path: &str) -> Result<()> {
    let p = std::path::Path::new(path);
    if p.is_absolute() {
        return Err(Error::FileTransfer("absolute path not allowed".into()));
    }
    if p.components()
        .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err(Error::FileTransfer(
            "path must be relative without '..'".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_roundtrip_plain_and_compressed() {
        let data: Vec<u8> = (0..100_000u32).map(|i| (i % 251) as u8).collect();
        for compress in [false, true] {
            let chunks = chunk_bytes("t", "f.bin", &data, 8192, compress);
            assert!(chunks.len() > 1);
            assert_eq!(reassemble(&chunks).unwrap(), data);
        }
    }

    #[test]
    fn chunks_detect_tamper_and_reorder() {
        let data = vec![9u8; 10_000];
        let mut chunks = chunk_bytes("t", "f.bin", &data, 1024, false);
        chunks[1].data[0] ^= 0xff;
        assert!(reassemble(&chunks).is_err());
        let mut chunks = chunk_bytes("t", "f.bin", &data, 1024, false);
        chunks.swap(0, 1);
        assert!(reassemble(&chunks).is_err());
    }

    #[test]
    fn rel_path_checks() {
        assert!(check_rel_path("a/b.bin").is_ok());
        assert!(check_rel_path("../x").is_err());
        assert!(check_rel_path("/etc/passwd").is_err());
    }
}
