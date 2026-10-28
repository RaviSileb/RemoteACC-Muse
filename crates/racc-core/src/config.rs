//! Konfigurace `config.toml` (sekce dle kapitoly 4 zadání).

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::error::{Error, Result};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub bootstrap: BootstrapSection,
    pub device: DeviceSection,
    pub security: SecuritySection,
    pub performance: PerformanceSection,
    pub services: ServicesSection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BootstrapSection {
    pub host: String,
    pub port: u16,
    pub pinned_fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DeviceSection {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SecuritySection {
    pub pairing_pin: String,
    pub trusted_fingerprints: Vec<String>,
    pub trusted_devices: Vec<String>,
    pub tofu: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PerformanceSection {
    pub max_fps: u32,
    pub codec: String,
    pub hw_accel: bool,
    pub direct_p2p_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServicesSection {
    pub tty: bool,
    pub file_transfer: bool,
    pub video: bool,
    pub audio: bool,
}

impl Default for BootstrapSection {
    fn default() -> Self {
        Self {
            host: "vps.example.com".to_string(),
            port: 8443,
            pinned_fingerprint: String::new(),
        }
    }
}

impl Default for DeviceSection {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: "remoteacc-device".to_string(),
        }
    }
}

impl Default for SecuritySection {
    fn default() -> Self {
        Self {
            pairing_pin: String::new(),
            trusted_fingerprints: Vec::new(),
            trusted_devices: Vec::new(),
            tofu: true,
        }
    }
}

impl Default for PerformanceSection {
    fn default() -> Self {
        Self {
            max_fps: 60,
            codec: "auto".to_string(),
            hw_accel: true,
            direct_p2p_only: false,
        }
    }
}

impl Default for ServicesSection {
    fn default() -> Self {
        Self {
            tty: true,
            file_transfer: true,
            video: false,
            audio: false,
        }
    }
}

impl Config {
    /// Načte konfiguraci ze souboru TOML.
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).map_err(|e| Error::Config(e.to_string()))?;
        toml::from_str(&text).map_err(|e| Error::Config(e.to_string()))
    }

    /// Uloží konfiguraci do souboru TOML.
    pub fn save(&self, path: &Path) -> Result<()> {
        let text = toml::to_string_pretty(self).map_err(|e| Error::Config(e.to_string()))?;
        std::fs::write(path, text).map_err(|e| Error::Config(e.to_string()))?;
        Ok(())
    }

    /// Základní validace hodnot.
    pub fn validate(&self) -> Result<()> {
        if self.bootstrap.port == 0 {
            return Err(Error::Config("bootstrap.port must be non-zero".into()));
        }
        if self.performance.max_fps == 0 || self.performance.max_fps > 240 {
            return Err(Error::Config("performance.max_fps must be 1..=240".into()));
        }
        match self.performance.codec.as_str() {
            "auto" | "av1" | "hevc" | "h264" | "vp9" | "vp8" => {}
            other => return Err(Error::Config(format!("unknown codec '{other}'"))),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_validate() {
        Config::default().validate().unwrap();
    }

    #[test]
    fn roundtrip_toml() {
        let cfg = Config::default();
        let text = toml::to_string(&cfg).unwrap();
        let back: Config = toml::from_str(&text).unwrap();
        assert_eq!(back.performance.max_fps, 60);
        assert_eq!(back.bootstrap.port, 8443);
        assert!(back.services.tty);
    }

    #[test]
    fn rejects_bad_codec_and_port() {
        let mut cfg = Config::default();
        cfg.performance.codec = "divx".into();
        assert!(cfg.validate().is_err());
        let mut cfg = Config::default();
        cfg.bootstrap.port = 0;
        assert!(cfg.validate().is_err());
    }
}
