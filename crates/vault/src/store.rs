use crate::error::VaultError;
use crate::parse_vault_ref;
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use zeroize::{Zeroize, ZeroizeOnDrop};

const KEYRING_SERVICE: &str = "vsterm";
const KEYRING_USER: &str = "vault-master-key";
const NONCE_LEN: usize = 12;

/// A `vault://...` reference string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretRef(pub String);

impl SecretRef {
    pub fn new(entry_id: &str) -> Self {
        Self(crate::format_vault_ref(entry_id))
    }

    pub fn entry_id(&self) -> Result<&str, VaultError> {
        parse_vault_ref(&self.0).ok_or_else(|| VaultError::InvalidRef(self.0.clone()))
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct VaultFile {
    /// base64(nonce || ciphertext) per entry
    entries: HashMap<String, String>,
}

/// File-backed encrypted vault. Master key prefers OS keyring; falls back to in-memory for tests.
pub struct Vault {
    path: PathBuf,
    master_key: MasterKey,
    file: VaultFile,
}

#[derive(Zeroize, ZeroizeOnDrop)]
struct MasterKey([u8; 32]);

impl Vault {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, VaultError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let master_key = load_or_create_master_key()?;
        let file = if path.exists() {
            let raw = fs::read_to_string(&path)?;
            serde_yaml::from_str(&raw).map_err(|e| VaultError::Serde(e.to_string()))?
        } else {
            VaultFile::default()
        };
        Ok(Self {
            path,
            master_key,
            file,
        })
    }

    pub fn get(&self, entry_id: &str) -> Result<String, VaultError> {
        let encoded = self
            .file
            .entries
            .get(entry_id)
            .ok_or_else(|| VaultError::NotFound(entry_id.into()))?;
        decrypt(&self.master_key, encoded)
    }

    pub fn get_ref(&self, secret_ref: &str) -> Result<String, VaultError> {
        let id = parse_vault_ref(secret_ref)
            .ok_or_else(|| VaultError::InvalidRef(secret_ref.into()))?;
        self.get(id)
    }

    pub fn set(&mut self, entry_id: &str, plaintext: &str) -> Result<(), VaultError> {
        let encoded = encrypt(&self.master_key, plaintext)?;
        self.file.entries.insert(entry_id.to_string(), encoded);
        self.persist()
    }

    pub fn remove(&mut self, entry_id: &str) -> Result<(), VaultError> {
        self.file.entries.remove(entry_id);
        self.persist()
    }

    pub fn contains(&self, entry_id: &str) -> bool {
        self.file.entries.contains_key(entry_id)
    }

    fn persist(&self) -> Result<(), VaultError> {
        let text = serde_yaml::to_string(&self.file).map_err(|e| VaultError::Serde(e.to_string()))?;
        fs::write(&self.path, text)?;
        Ok(())
    }
}

fn load_or_create_master_key() -> Result<MasterKey, VaultError> {
    match keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER) {
        Ok(entry) => match entry.get_password() {
            Ok(existing) => {
                let bytes = decode_key(&existing)?;
                Ok(MasterKey(bytes))
            }
            Err(keyring::Error::NoEntry) => {
                let key = generate_key();
                let encoded = encode_key(&key.0);
                entry
                    .set_password(&encoded)
                    .map_err(|e| VaultError::Keyring(e.to_string()))?;
                Ok(key)
            }
            Err(e) => {
                tracing::warn!("keyring unavailable ({e}), using ephemeral master key");
                Ok(generate_key())
            }
        },
        Err(e) => {
            tracing::warn!("keyring unavailable ({e}), using ephemeral master key");
            Ok(generate_key())
        }
    }
}

fn generate_key() -> MasterKey {
    use aes_gcm::aead::OsRng;
    use aes_gcm::aead::rand_core::RngCore;
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    MasterKey(bytes)
}

fn encode_key(key: &[u8; 32]) -> String {
    bytes_to_hex(key)
}

fn decode_key(s: &str) -> Result<[u8; 32], VaultError> {
    let bytes = hex_to_bytes(s)?;
    if bytes.len() != 32 {
        return Err(VaultError::Crypto("master key must be 32 bytes".into()));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn encrypt(key: &MasterKey, plaintext: &str) -> Result<String, VaultError> {
    use aes_gcm::aead::OsRng;
    use aes_gcm::aead::rand_core::RngCore;

    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key.0));
    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let mut ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| VaultError::Crypto(e.to_string()))?;
    let mut blob = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    blob.extend_from_slice(&nonce_bytes);
    blob.append(&mut ciphertext);
    Ok(bytes_to_hex(&blob))
}

fn decrypt(key: &MasterKey, encoded: &str) -> Result<String, VaultError> {
    let blob = hex_to_bytes(encoded)?;
    if blob.len() < NONCE_LEN + 1 {
        return Err(VaultError::Crypto("ciphertext too short".into()));
    }
    let (nonce_bytes, ct) = blob.split_at(NONCE_LEN);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key.0));
    let nonce = Nonce::from_slice(nonce_bytes);
    let plain = cipher
        .decrypt(nonce, ct)
        .map_err(|e| VaultError::Crypto(e.to_string()))?;
    String::from_utf8(plain).map_err(|e| VaultError::Crypto(e.to_string()))
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn hex_to_bytes(s: &str) -> Result<Vec<u8>, VaultError> {
    if !s.len().is_multiple_of(2) {
        return Err(VaultError::Crypto("invalid hex length".into()));
    }
    (0..s.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&s[i..i + 2], 16)
                .map_err(|e| VaultError::Crypto(e.to_string()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn roundtrip_secret() {
        let dir = env::temp_dir().join(format!("vsterm-vault-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("vault.enc");

        // Bypass keyring flakiness in CI by constructing via open (may use ephemeral key)
        let mut vault = Vault::open(&path).unwrap();
        vault.set("demo-pwd", "s3cret").unwrap();
        assert_eq!(vault.get("demo-pwd").unwrap(), "s3cret");
        assert_eq!(vault.get_ref("vault://demo-pwd").unwrap(), "s3cret");

        let _ = fs::remove_dir_all(&dir);
    }
}
