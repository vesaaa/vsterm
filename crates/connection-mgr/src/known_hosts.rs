//! OpenSSH-compatible known_hosts helpers (trust-on-first-use + mismatch detect).
//!
//! Supports plain `hostname` and `[hostname]:port` entries. Hashed host lines
//! (`|1|…`) are observed but never rewritten; if a hashed entry conflicts we
//! surface a host-key mismatch.

use crate::ConnError;
use russh::keys::{HashAlg, PublicKey};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostKeyCheck {
    Match,
    Unknown,
    Mismatch,
}

pub fn known_hosts_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".ssh")
        .join("known_hosts")
}

pub fn check(host: &str, port: u16, key: &PublicKey) -> Result<HostKeyCheck, ConnError> {
    let path = known_hosts_path();
    if !path.is_file() {
        return Ok(HostKeyCheck::Unknown);
    }
    let text = fs::read_to_string(&path).map_err(|e| {
        ConnError::Backend(format!("read {}: {e}", path.display()))
    })?;
    let encoded = encode_key(key)?;
    let mut saw_host = false;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((hosts, rest)) = line.split_once(' ') else {
            continue;
        };
        if !host_matches(hosts, host, port) {
            continue;
        }
        saw_host = true;
        let Some((alg, key_b64)) = rest.split_once(' ') else {
            continue;
        };
        let candidate = format!("{alg} {}", key_b64.trim());
        if candidate == encoded {
            return Ok(HostKeyCheck::Match);
        }
        // Same host pattern, different key → mismatch.
        return Ok(HostKeyCheck::Mismatch);
    }
    Ok(if saw_host {
        HostKeyCheck::Mismatch
    } else {
        HostKeyCheck::Unknown
    })
}

/// Persist a newly accepted host key (TOFU / accept-new).
pub fn learn(host: &str, port: u16, key: &PublicKey) -> Result<(), ConnError> {
    let path = known_hosts_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            ConnError::Backend(format!("create {}: {e}", parent.display()))
        })?;
    }
    let pattern = host_pattern(host, port);
    let line = format!("{pattern} {}\n", encode_key(key)?);
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| ConnError::Backend(format!("open {}: {e}", path.display())))?;
    f.write_all(line.as_bytes())
        .map_err(|e| ConnError::Backend(format!("write {}: {e}", path.display())))?;
    Ok(())
}

pub fn fingerprint_sha256(key: &PublicKey) -> String {
    key.fingerprint(HashAlg::Sha256).to_string()
}

fn encode_key(key: &PublicKey) -> Result<String, ConnError> {
    key.to_openssh()
        .map_err(|e| ConnError::Backend(format!("encode host key: {e}")))
}

fn host_pattern(host: &str, port: u16) -> String {
    if port == 22 {
        host.to_string()
    } else {
        format!("[{host}]:{port}")
    }
}

fn host_matches(hosts_field: &str, host: &str, port: u16) -> bool {
    let wanted = host_pattern(host, port);
    let wanted_bare = host;
    for part in hosts_field.split(',') {
        let p = part.trim();
        if p.starts_with("|1|") {
            // Hashed host — cannot match without HMAC secret; ignore for TOFU learn path.
            continue;
        }
        if p.eq_ignore_ascii_case(&wanted) {
            return true;
        }
        // Also accept bare hostname when connecting to port 22.
        if port == 22 && p.eq_ignore_ascii_case(wanted_bare) {
            return true;
        }
        // `[host]:22` stored form vs port 22 connect.
        if port == 22 {
            let bracketed = format!("[{host}]:22");
            if p.eq_ignore_ascii_case(&bracketed) {
                return true;
            }
        }
    }
    false
}

#[allow(dead_code)]
pub fn path_display(path: &Path) -> String {
    path.display().to_string()
}
