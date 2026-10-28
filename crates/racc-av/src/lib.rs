//! racc-av: audio/video subsystém RemoteACC.
//!
//! Fáze MVP (1–2): čistá logika – dělení snímku na dlaždice 64×64,
//! kaskáda kodeků, BBR-inspirovaný FPS regulátor, client-side kurzor.
//! OS zachytávání (DXGI / PipeWire / ScreenCaptureKit / MediaProjection /
//! ReplayKit) přichází ve fázi 3 za feature flagy – viz [`backend`].

pub mod backend;
pub mod codec;
pub mod cursor;
pub mod tiles;

pub use backend::{BackendKind, CaptureBackend, MockSource, RawFrame};
pub use codec::{select_codec, Codec, FpsController};
pub use cursor::CursorState;
pub use tiles::{diff_tiles, merge_row_rects, TileRect};
