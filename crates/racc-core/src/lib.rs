//! racc-core: společné jádro RemoteACC.
//!
//! Obsahuje: konfiguraci, `racc://` URI, kryptografii (Ed25519 identity,
//! X25519 handshake, ChaCha20-Poly1305 session cipher), Protobuf protokol,
//! QUIC transportní pomocníky, chunkovaný přenos souborů a C-ABI pro mobilní/GUI klienty.

pub mod config;
pub mod crypto;
pub mod error;
pub mod ffi;
pub mod net;
pub mod proto;
pub mod uri;

pub use config::Config;
pub use error::{Error, Result};
