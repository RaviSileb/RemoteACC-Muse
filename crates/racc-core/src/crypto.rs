//! Kryptografie: Ed25519 identity, X25519 handshake, ChaCha20-Poly1305 session cipher.
//!
//! MVP implementuje handshake ve stylu Noise XX (zjednodušeně, dokumentováno
//! v `docs/PROTOKOL.md`). Plná integrace Noise Framework je plánována ve fázi 2.

use chacha20poly1305::{
    aead::{Aead, OsRng as AeadOsRng},
    AeadCore, ChaCha20Poly1305, KeyInit,
};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand_core::OsRng;
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey as XPublic, StaticSecret};

use crate::error::{Error, Result};

/// Dlouhodobá identita zařízení (Ed25519).
pub struct Identity {
    signing: SigningKey,
}

impl Identity {
    pub fn generate() -> Self {
        Self {
            signing: SigningKey::generate(&mut OsRng),
        }
    }

    pub fn from_secret_bytes(bytes: &[u8; 32]) -> Self {
        Self {
            signing: SigningKey::from_bytes(bytes),
        }
    }

    pub fn secret_bytes(&self) -> [u8; 32] {
        self.signing.to_bytes()
    }

    pub fn verifying(&self) -> VerifyingKey {
        self.signing.verifying_key()
    }

    /// Otisk identity: hex(SHA-256(verifying-key)). Používá se v `racc://` URI.
    pub fn fingerprint(&self) -> String {
        fingerprint_of(&self.verifying().to_bytes())
    }

    pub fn sign(&self, msg: &[u8]) -> Signature {
        self.signing.sign(msg)
    }

    pub fn verify(vk_bytes: &[u8; 32], msg: &[u8], sig_bytes: &[u8; 64]) -> Result<()> {
        let vk = VerifyingKey::from_bytes(vk_bytes).map_err(|e| Error::Crypto(e.to_string()))?;
        let sig = Signature::from_bytes(sig_bytes);
        vk.verify(msg, &sig)
            .map_err(|e| Error::Crypto(e.to_string()))
    }
}

/// Otisk libovolných veřejných bajtů identity.
pub fn fingerprint_of(public_bytes: &[u8]) -> String {
    let digest = Sha256::digest(public_bytes);
    hex::encode(digest)
}

/// Efemérní X25519 klíč pro jedno sezení.
pub struct Ephemeral {
    secret: StaticSecret,
    public: XPublic,
}

impl Ephemeral {
    pub fn generate() -> Self {
        let secret = StaticSecret::random_from_rng(OsRng);
        let public = XPublic::from(&secret);
        Self { secret, public }
    }

    pub fn public_bytes(&self) -> [u8; 32] {
        *self.public.as_bytes()
    }

    pub fn diffie_hellman(&self, peer: &[u8; 32]) -> [u8; 32] {
        let peer = XPublic::from(*peer);
        self.secret.diffie_hellman(&peer).to_bytes()
    }
}

/// Odvození klíče sezení: SHA-256(shared || init_epub || resp_epub).
pub fn derive_session_key(
    shared: &[u8; 32],
    init_epub: &[u8; 32],
    resp_epub: &[u8; 32],
) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(shared);
    h.update(init_epub);
    h.update(resp_epub);
    h.finalize().into()
}

/// První zpráva handshake (iniciátor -> respondér).
#[derive(Debug, Clone)]
pub struct HandshakeInit {
    pub identity_pub: [u8; 32],
    pub ephemeral_pub: [u8; 32],
    pub signature: [u8; 64],
}

/// Druhá zpráva handshake (respondér -> iniciátor).
#[derive(Debug, Clone)]
pub struct HandshakeResp {
    pub identity_pub: [u8; 32],
    pub ephemeral_pub: [u8; 32],
    pub signature: [u8; 64],
}

/// Vytvoří první zprávu handshake a vrátí ji spolu s efemérním klíčem.
pub fn handshake_initiate(id: &Identity) -> (Ephemeral, HandshakeInit) {
    let eph = Ephemeral::generate();
    let sig = id.sign(&eph.public_bytes());
    let init = HandshakeInit {
        identity_pub: id.verifying().to_bytes(),
        ephemeral_pub: eph.public_bytes(),
        signature: sig.to_bytes(),
    };
    (eph, init)
}

/// Zpracuje první zprávu, vytvoří odpověď a odvodí klíč sezení.
/// Vrací (odpověď, klíč sezení, ověřený otisk iniciátora).
pub fn handshake_respond(
    id: &Identity,
    init: &HandshakeInit,
) -> Result<(HandshakeResp, [u8; 32], String)> {
    verify_init(init)?;
    let eph = Ephemeral::generate();
    let sig = id.sign(&eph.public_bytes());
    let resp = HandshakeResp {
        identity_pub: id.verifying().to_bytes(),
        ephemeral_pub: eph.public_bytes(),
        signature: sig.to_bytes(),
    };
    let shared = eph.diffie_hellman(&init.ephemeral_pub);
    let key = derive_session_key(&shared, &init.ephemeral_pub, &resp.ephemeral_pub);
    Ok((resp, key, fingerprint_of(&init.identity_pub)))
}

fn verify_init(init: &HandshakeInit) -> Result<()> {
    Identity::verify(&init.identity_pub, &init.ephemeral_pub, &init.signature)
}

/// Dokončí handshake na straně iniciátora.
/// Vrací (klíč sezení, ověřený otisk respondéra).
pub fn handshake_finish(
    eph: &Ephemeral,
    init: &HandshakeInit,
    resp: &HandshakeResp,
) -> Result<([u8; 32], String)> {
    Identity::verify(&resp.identity_pub, &resp.ephemeral_pub, &resp.signature)?;
    let shared = eph.diffie_hellman(&resp.ephemeral_pub);
    let key = derive_session_key(&shared, &init.ephemeral_pub, &resp.ephemeral_pub);
    Ok((key, fingerprint_of(&resp.identity_pub)))
}

/// Šifra sezení ChaCha20-Poly1305. Každá zpráva má náhodný 12B nonce,
/// výstup je `nonce || ciphertext`.
pub struct SessionCipher {
    cipher: ChaCha20Poly1305,
}

impl SessionCipher {
    pub fn new(key: &[u8; 32]) -> Self {
        Self {
            cipher: ChaCha20Poly1305::new(key.into()),
        }
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> Vec<u8> {
        let nonce = ChaCha20Poly1305::generate_nonce(&mut AeadOsRng);
        let mut out = Vec::with_capacity(12 + plaintext.len() + 16);
        out.extend_from_slice(&nonce);
        out.extend_from_slice(
            &self
                .cipher
                .encrypt(&nonce, plaintext)
                .expect("encryption cannot fail"),
        );
        out
    }

    pub fn decrypt(&self, packet: &[u8]) -> Result<Vec<u8>> {
        if packet.len() < 12 + 16 {
            return Err(Error::Crypto("packet too short".into()));
        }
        let (nonce, ct) = packet.split_at(12);
        self.cipher
            .decrypt(nonce.into(), ct)
            .map_err(|_| Error::Crypto("decryption failed (bad key or tampered data)".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_sign_verify_roundtrip() {
        let id = Identity::generate();
        let msg = b"remoteacc";
        let sig = id.sign(msg);
        Identity::verify(&id.verifying().to_bytes(), msg, &sig.to_bytes()).unwrap();
        assert_eq!(id.fingerprint().len(), 64);
    }

    #[test]
    fn handshake_both_sides_agree() {
        let a = Identity::generate();
        let b = Identity::generate();
        let (eph_a, init) = handshake_initiate(&a);
        let (resp, key_b, fp_a_seen) = handshake_respond(&b, &init).unwrap();
        let (key_a, fp_b_seen) = handshake_finish(&eph_a, &init, &resp).unwrap();
        assert_eq!(key_a, key_b);
        assert_eq!(fp_a_seen, a.fingerprint());
        assert_eq!(fp_b_seen, b.fingerprint());
    }

    #[test]
    fn handshake_rejects_forged_signature() {
        let a = Identity::generate();
        let b = Identity::generate();
        let (_, mut init) = handshake_initiate(&a);
        init.signature[0] ^= 0xff;
        assert!(handshake_respond(&b, &init).is_err());
    }

    #[test]
    fn cipher_roundtrip_and_tamper() {
        let c = SessionCipher::new(&[7u8; 32]);
        let ct = c.encrypt(b"hello p2p");
        assert_eq!(c.decrypt(&ct).unwrap(), b"hello p2p");
        let mut bad = ct.clone();
        let n = bad.len();
        bad[n - 1] ^= 0x01;
        assert!(c.decrypt(&bad).is_err());
        assert!(c.decrypt(&ct[..5]).is_err());
    }
}
