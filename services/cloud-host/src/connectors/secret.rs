use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine;
use rand::RngCore;

const NONCE_LEN: usize = 12;

pub struct ConnectorSecretBox {
    cipher: Aes256Gcm,
}

impl ConnectorSecretBox {
    pub fn from_base64_key(encoded: &str) -> Result<Self, String> {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded.trim())
            .map_err(|e| format!("invalid connector secret key encoding: {e}"))?;
        if bytes.len() != 32 {
            return Err("ELSEWHERE_CONNECTOR_SECRET_KEY must decode to 32 bytes".into());
        }
        let cipher = Aes256Gcm::new_from_slice(&bytes)
            .map_err(|e| format!("invalid connector secret key: {e}"))?;
        Ok(Self { cipher })
    }

    pub fn encrypt(&self, plaintext: &str) -> Result<(Vec<u8>, Vec<u8>), String> {
        let mut nonce = [0u8; NONCE_LEN];
        rand::thread_rng().fill_bytes(&mut nonce);
        let ciphertext = self
            .cipher
            .encrypt(Nonce::from_slice(&nonce), plaintext.as_bytes())
            .map_err(|e| format!("encrypt failed: {e}"))?;
        Ok((nonce.to_vec(), ciphertext))
    }

    pub fn decrypt(&self, nonce: &[u8], ciphertext: &[u8]) -> Result<String, String> {
        if nonce.len() != NONCE_LEN {
            return Err("invalid connector secret nonce".into());
        }
        let plain = self
            .cipher
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|_| "could not decrypt connector secret".to_string())?;
        String::from_utf8(plain).map_err(|_| "connector secret is not valid UTF-8".into())
    }
}
