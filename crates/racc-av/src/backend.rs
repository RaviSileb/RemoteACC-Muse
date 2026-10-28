//! Zachytávací backendy. MVP dodává [`MockSource`] pro testy;
//! OS backendy (DXGI / PipeWire+xdg-portal / ScreenCaptureKit /
//! MediaProjection / ReplayKit) se zapojí ve fázi 3 za feature flagy.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum BackendError {
    #[error("backend '{0}' není v tomto buildu dostupný")]
    Unavailable(&'static str),
    #[error("capture selhalo: {0}")]
    Capture(String),
}

/// Druh zachytávacího backendu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    Mock,
    Dxgi,
    PipeWire,
    ScreenCaptureKit,
    MediaProjection,
    ReplayKit,
}

impl BackendKind {
    /// Vybere nejlepší backend pro aktuální build.
    pub fn native() -> Self {
        #[cfg(feature = "win-dxgi")]
        return Self::Dxgi;
        #[cfg(feature = "linux-pipewire")]
        return Self::PipeWire;
        #[cfg(feature = "macos-sck")]
        return Self::ScreenCaptureKit;
        #[cfg(not(any(
            feature = "win-dxgi",
            feature = "linux-pipewire",
            feature = "macos-sck"
        )))]
        Self::Mock
    }
}

/// Syrový snímek (RGBA).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawFrame {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl RawFrame {
    pub fn bytes_per_pixel() -> u32 {
        4
    }
}

/// Zdroj snímků.
pub trait CaptureBackend {
    fn kind(&self) -> BackendKind;
    fn capture(&mut self) -> std::result::Result<RawFrame, BackendError>;
}

/// Testovací zdroj generující šachovnici s posuvem.
pub struct MockSource {
    width: u32,
    height: u32,
    tick: u32,
}

impl MockSource {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            tick: 0,
        }
    }
}

impl CaptureBackend for MockSource {
    fn kind(&self) -> BackendKind {
        BackendKind::Mock
    }

    fn capture(&mut self) -> std::result::Result<RawFrame, BackendError> {
        self.tick += 1;
        let mut data = vec![0u8; (self.width * self.height * 4) as usize];
        for (i, px) in data.chunks_exact_mut(4).enumerate() {
            let x = (i as u32) % self.width;
            let y = (i as u32) / self.width;
            let v = if (x / 16 + y / 16 + self.tick) % 2 == 0 {
                0x20
            } else {
                0xe0
            };
            px.copy_from_slice(&[v, v, v, 0xff]);
        }
        Ok(RawFrame {
            width: self.width,
            height: self.height,
            data,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_produces_changing_frames() {
        let mut src = MockSource::new(64, 64);
        let a = src.capture().unwrap();
        let b = src.capture().unwrap();
        assert_ne!(a.data, b.data);
        assert_eq!(a.data.len(), 64 * 64 * 4);
    }
}
