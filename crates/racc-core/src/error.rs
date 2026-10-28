use thiserror::Error;

/// Sjednocená chyba jádra RemoteACC.
#[derive(Debug, Error)]
pub enum Error {
    #[error("config: {0}")]
    Config(String),
    #[error("uri: {0}")]
    Uri(String),
    #[error("crypto: {0}")]
    Crypto(String),
    #[error("transport: {0}")]
    Transport(String),
    #[error("proto: {0}")]
    Proto(String),
    #[error("file transfer: {0}")]
    FileTransfer(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
