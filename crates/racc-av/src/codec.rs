//! Kaskáda kodeků a FPS regulátor.
//!
//! Kaskáda ze zadání: AV1 > HEVC/H.264 (YUV420p/YUV444) > VP9/VP8.
//! BBR princip: při zahlcení se snižuje FPS, ne kvalita textu.

use serde::{Deserialize, Serialize};

/// Podporovaný video kodek.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Codec {
    Av1,
    Hevc,
    H264,
    Vp9,
    Vp8,
    Raw,
}

impl Codec {
    pub fn as_str(self) -> &'static str {
        match self {
            Codec::Av1 => "av1",
            Codec::Hevc => "hevc",
            Codec::H264 => "h264",
            Codec::Vp9 => "vp9",
            Codec::Vp8 => "vp8",
            Codec::Raw => "raw",
        }
    }
}

/// Výběr kodeku: `preferred` = "auto" nebo konkrétní kodek.
/// Při `hw_accel` se preferuje kodek s širokou HW podporou (H.264),
/// jinak AV1 (nejlepší komprese pro text).
pub fn select_codec(preferred: &str, hw_accel: bool) -> Codec {
    match preferred {
        "av1" => Codec::Av1,
        "hevc" => Codec::Hevc,
        "h264" => Codec::H264,
        "vp9" => Codec::Vp9,
        "vp8" => Codec::Vp8,
        "raw" => Codec::Raw,
        _ if hw_accel => Codec::H264,
        _ => Codec::Av1,
    }
}

/// FPS regulátor inspirovaný BBR: při ztrátách/RTT snižuje snímkovou
/// frekvenci (nikoli kvalitu), při dobré síti se vrací k cíli.
#[derive(Debug, Clone)]
pub struct FpsController {
    target: u32,
    current: u32,
    min: u32,
    good_windows: u32,
}

impl FpsController {
    pub fn new(target: u32) -> Self {
        Self {
            target: target.clamp(1, 240),
            current: target.clamp(1, 240),
            min: 5,
            good_windows: 0,
        }
    }

    pub fn current(&self) -> u32 {
        self.current
    }

    /// `loss01` = podíl ztracených paketů 0.0–1.0; `rtt_ms` = odhad RTT.
    pub fn on_sample(&mut self, loss01: f32, rtt_ms: u64) {
        if loss01 > 0.05 || rtt_ms > 250 {
            // Špatná síť: okamžitě dolů (multiplikativně), min. 5 FPS.
            self.current = (self.current * 3 / 4).max(self.min);
            self.good_windows = 0;
        } else if loss01 < 0.01 && rtt_ms < 120 {
            // Dobrá síť: pomalý návrat nahoru (aditivně).
            self.good_windows += 1;
            if self.good_windows >= 3 {
                self.current = (self.current + 5).min(self.target);
                self.good_windows = 0;
            }
        } else {
            self.good_windows = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codec_cascade() {
        assert_eq!(select_codec("auto", true), Codec::H264);
        assert_eq!(select_codec("auto", false), Codec::Av1);
        assert_eq!(select_codec("vp9", true), Codec::Vp9);
    }

    #[test]
    fn fps_drops_on_loss_and_recovers() {
        let mut c = FpsController::new(60);
        c.on_sample(0.2, 400);
        assert!(c.current() < 60);
        let dropped = c.current();
        for _ in 0..9 {
            c.on_sample(0.0, 30);
        }
        assert!(c.current() > dropped);
    }

    #[test]
    fn fps_never_below_min() {
        let mut c = FpsController::new(60);
        for _ in 0..20 {
            c.on_sample(0.9, 2000);
        }
        assert_eq!(c.current(), 5);
    }
}
