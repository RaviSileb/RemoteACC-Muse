//! Protobuf protokol datové roviny (prost, bez `protoc` – zprávy jako derive).
//!
//! Rozhodnutí optimalizace: **Protobuf (prost)** místo FlatBuffers –
//! jednodušší build (žádný `protoc` v CI), dostatečný výkon pro řídicí
//! zprávy a TTY/soubory; FlatBuffers zůstává možnost pro video ve fázi 3.

use crate::error::{Error, Result};

/// Verze obálky. Příjemce odmítne jinou major verzi.
pub const ENVELOPE_VERSION: u32 = 1;

/// Typ obsahu obálky.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, prost::Enumeration)]
#[repr(i32)]
pub enum Kind {
    Unspecified = 0,
    Tty = 1,
    Video = 2,
    Audio = 3,
    File = 4,
    Signal = 5,
    Keepalive = 6,
}

/// Sjednocující obálka všech zpráv datové roviny.
#[derive(Clone, PartialEq, prost::Message)]
pub struct Envelope {
    #[prost(uint32, tag = "1")]
    pub version: u32,
    /// Náhodné ID sezení (16 B).
    #[prost(bytes = "vec", tag = "2")]
    pub session_id: Vec<u8>,
    #[prost(enumeration = "Kind", tag = "3")]
    pub kind: i32,
    /// Serializovaná vnitřní zpráva (`TtyData`, `FileChunk`, …).
    #[prost(bytes = "vec", tag = "4")]
    pub payload: Vec<u8>,
}

/// TTY data (obousměrně). `resize` je Some jen u řídicí zprávy.
#[derive(Clone, PartialEq, prost::Message)]
pub struct TtyData {
    #[prost(uint32, tag = "1")]
    pub pty_id: u32,
    #[prost(bytes = "vec", tag = "2")]
    pub data: Vec<u8>,
    #[prost(message, optional, tag = "3")]
    pub resize: Option<TtyResize>,
    /// true = EOF / konec PTY.
    #[prost(bool, tag = "4")]
    pub eof: bool,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct TtyResize {
    #[prost(uint32, tag = "1")]
    pub cols: u32,
    #[prost(uint32, tag = "2")]
    pub rows: u32,
}

/// Jeden chunk přenosu souboru.
#[derive(Clone, PartialEq, prost::Message)]
pub struct FileChunk {
    #[prost(string, tag = "1")]
    pub transfer_id: String,
    /// Relativní cesta cílového souboru (kontroluje se proti `..`).
    #[prost(string, tag = "2")]
    pub path: String,
    #[prost(uint64, tag = "3")]
    pub index: u64,
    #[prost(uint64, tag = "4")]
    pub total_chunks: u64,
    /// SHA-256 nekomprimovaných dat chunku (32 B).
    #[prost(bytes = "vec", tag = "5")]
    pub sha256: Vec<u8>,
    /// true = `data` jsou komprimována zstd.
    #[prost(bool, tag = "6")]
    pub compressed: bool,
    #[prost(bytes = "vec", tag = "7")]
    pub data: Vec<u8>,
    /// true = poslední chunk souboru.
    #[prost(bool, tag = "8")]
    pub last: bool,
}

/// Signalizační zpráva (přes bootstrap nebo přímo).
#[derive(Clone, PartialEq, prost::Message)]
pub struct SignalMsg {
    #[prost(string, tag = "1")]
    pub from: String,
    #[prost(string, tag = "2")]
    pub to: String,
    /// JSON tělo (`{"op": ...}`, viz `docs/PROTOKOL.md`).
    #[prost(string, tag = "3")]
    pub body_json: String,
}

/// Metadata video snímku (samotná data jdou jako navazující binární rámec).
#[derive(Clone, PartialEq, prost::Message)]
pub struct VideoFrame {
    #[prost(uint32, tag = "1")]
    pub width: u32,
    #[prost(uint32, tag = "2")]
    pub height: u32,
    /// Kodek: "av1" | "hevc" | "h264" | "vp9" | "vp8" | "raw".
    #[prost(string, tag = "3")]
    pub codec: String,
    /// Počet dirty dlaždic v tomto snímku.
    #[prost(uint32, tag = "4")]
    pub tiles: u32,
    #[prost(uint64, tag = "5")]
    pub seq: u64,
}

/// Metadata audio rámce (Opus).
#[derive(Clone, PartialEq, prost::Message)]
pub struct AudioFrame {
    /// Vzorkovací frekvence, např. 48000.
    #[prost(uint32, tag = "1")]
    pub sample_rate: u32,
    #[prost(uint32, tag = "2")]
    pub channels: u32,
    #[prost(uint64, tag = "3")]
    pub seq: u64,
}

impl Envelope {
    /// Serializuje obálku do bajtů.
    pub fn encode_vec(&self) -> Vec<u8> {
        use prost::Message;
        self.encode_to_vec()
    }

    /// Deserializuje obálku z bajtů.
    pub fn decode_vec(bytes: &[u8]) -> Result<Self> {
        use prost::Message;
        Self::decode(bytes).map_err(|e| Error::Proto(e.to_string()))
    }

    /// Zabalte vnitřní zprávu do obálky daného druhu.
    pub fn wrap<M: prost::Message>(kind: Kind, session_id: &[u8], msg: &M) -> Self {
        Self {
            version: ENVELOPE_VERSION,
            session_id: session_id.to_vec(),
            kind: kind as i32,
            payload: msg.encode_to_vec(),
        }
    }

    /// Ověří verzi a rozbalí vnitřní zprávu.
    pub fn unwrap<M: prost::Message + Default>(&self, expect: Kind) -> Result<M> {
        if self.version != ENVELOPE_VERSION {
            return Err(Error::Proto(format!(
                "unsupported envelope version {}",
                self.version
            )));
        }
        let kind = Kind::try_from(self.kind)
            .map_err(|_| Error::Proto(format!("unknown kind {}", self.kind)))?;
        if kind != expect {
            return Err(Error::Proto(format!(
                "expected kind {expect:?}, got {kind:?}"
            )));
        }
        M::decode(&*self.payload).map_err(|e| Error::Proto(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost::Message;

    #[test]
    fn envelope_wrap_unwrap() {
        let tty = TtyData {
            pty_id: 3,
            data: b"ls\n".to_vec(),
            resize: None,
            eof: false,
        };
        let env = Envelope::wrap(Kind::Tty, b"session-16-bytes", &tty);
        let back: TtyData = env.clone().unwrap(Kind::Tty).unwrap();
        assert_eq!(back, tty);

        let bytes = env.encode_to_vec();
        let decoded = Envelope::decode(&*bytes).unwrap();
        let back2: TtyData = decoded.unwrap(Kind::Tty).unwrap();
        assert_eq!(back2, tty);
    }

    #[test]
    fn rejects_wrong_kind_and_version() {
        let tty = TtyData::default();
        let mut env = Envelope::wrap(Kind::Tty, b"s", &tty);
        assert!(env.clone().unwrap::<TtyData>(Kind::File).is_err());
        env.version = 999;
        assert!(env.unwrap::<TtyData>(Kind::Tty).is_err());
    }

    #[test]
    fn file_chunk_roundtrip() {
        let c = FileChunk {
            transfer_id: "t1".into(),
            path: "a/b.bin".into(),
            index: 0,
            total_chunks: 1,
            sha256: vec![0u8; 32],
            compressed: false,
            data: vec![1, 2, 3],
            last: true,
        };
        let env = Envelope::wrap(Kind::File, b"s", &c);
        let back: FileChunk = env.unwrap(Kind::File).unwrap();
        assert_eq!(back, c);
    }
}
