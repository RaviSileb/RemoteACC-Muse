//! `racc://` URI: `racc://[host]:[port]/[device-id]#fingerprint`

use std::fmt;
use std::str::FromStr;

use crate::error::{Error, Result};

/// Identifikátor vzdáleného zařízení včetně bootstrap cíle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RaccUri {
    pub host: String,
    pub port: u16,
    pub device_id: String,
    pub fingerprint: Option<String>,
}

impl FromStr for RaccUri {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let rest = s
            .strip_prefix("racc://")
            .ok_or_else(|| Error::Uri("missing 'racc://' scheme".into()))?;
        let (addr_part, fingerprint) = match rest.split_once('#') {
            Some((a, f)) => (a, Some(f.to_string())),
            None => (rest, None),
        };
        let (hostport, device_id) = addr_part
            .split_once('/')
            .ok_or_else(|| Error::Uri("missing '/<device-id>' part".into()))?;
        if device_id.is_empty() {
            return Err(Error::Uri("empty device-id".into()));
        }
        let (host, port) = hostport
            .rsplit_once(':')
            .ok_or_else(|| Error::Uri("missing ':<port>' part".into()))?;
        if host.is_empty() {
            return Err(Error::Uri("empty host".into()));
        }
        let port: u16 = port
            .parse()
            .map_err(|_| Error::Uri(format!("invalid port '{port}'")))?;
        if port == 0 {
            return Err(Error::Uri("port must be non-zero".into()));
        }
        if let Some(fp) = &fingerprint {
            if fp.is_empty() {
                return Err(Error::Uri("empty fingerprint after '#'".into()));
            }
        }
        Ok(Self {
            host: host.to_string(),
            port,
            device_id: device_id.to_string(),
            fingerprint,
        })
    }
}

impl fmt::Display for RaccUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "racc://{}:{}/{}", self.host, self.port, self.device_id)?;
        if let Some(fp) = &self.fingerprint {
            write!(f, "#{fp}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_uri() {
        let u: RaccUri = "racc://vps.example.com:8443/device-1#abc123"
            .parse()
            .unwrap();
        assert_eq!(u.host, "vps.example.com");
        assert_eq!(u.port, 8443);
        assert_eq!(u.device_id, "device-1");
        assert_eq!(u.fingerprint.as_deref(), Some("abc123"));
        assert_eq!(u.to_string(), "racc://vps.example.com:8443/device-1#abc123");
    }

    #[test]
    fn parse_without_fingerprint() {
        let u: RaccUri = "racc://10.0.0.1:8443/dev".parse().unwrap();
        assert_eq!(u.fingerprint, None);
    }

    #[test]
    fn rejects_malformed() {
        assert!("https://x:1/d".parse::<RaccUri>().is_err());
        assert!("racc://x/d".parse::<RaccUri>().is_err());
        assert!("racc://x:0/d".parse::<RaccUri>().is_err());
        assert!("racc://:1/d".parse::<RaccUri>().is_err());
        assert!("racc://x:1/".parse::<RaccUri>().is_err());
        assert!("racc://x:1/d#".parse::<RaccUri>().is_err());
    }
}
