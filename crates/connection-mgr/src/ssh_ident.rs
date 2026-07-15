//! Pre-auth SSH identification probe (TCP banner line).

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

const PROBE_TIMEOUT: Duration = Duration::from_secs(3);

/// Read the server's SSH identification string before any authentication.
///
/// Returns a display string such as `OpenSSH_9.6p1 Ubuntu-3ubuntu13.5`
/// (protocol prefix stripped). Failures are soft — callers show host only.
pub fn probe_ssh_software_ident(host: &str, port: u16) -> Result<String, String> {
    let addr = resolve_first(host, port)?;
    let mut stream = TcpStream::connect_timeout(&addr, PROBE_TIMEOUT)
        .map_err(|e| format!("connect: {e}"))?;
    stream
        .set_read_timeout(Some(PROBE_TIMEOUT))
        .map_err(|e| format!("set timeout: {e}"))?;
    stream
        .set_write_timeout(Some(PROBE_TIMEOUT))
        .map_err(|e| format!("set timeout: {e}"))?;

    let raw = read_ident_line(&mut stream)?;
    // Reciprocate with a minimal client ident so some servers close cleanly.
    let _ = stream.write_all(b"SSH-2.0-VsTerm_probe\r\n");
    let _ = stream.shutdown(std::net::Shutdown::Both);

    format_ident_display(&raw).ok_or_else(|| format!("unexpected ident: {raw}"))
}

fn resolve_first(host: &str, port: u16) -> Result<SocketAddr, String> {
    (host, port)
        .to_socket_addrs()
        .map_err(|e| format!("dns: {e}"))?
        .next()
        .ok_or_else(|| "dns: no addresses".to_string())
}

fn read_ident_line(stream: &mut TcpStream) -> Result<String, String> {
    let mut line = Vec::with_capacity(64);
    let mut byte = [0u8; 1];
    loop {
        match stream.read(&mut byte) {
            Ok(0) => break,
            Ok(_) => {
                if byte[0] == b'\n' {
                    break;
                }
                if byte[0] != b'\r' {
                    line.push(byte[0]);
                }
                if line.len() >= 255 {
                    break;
                }
            }
            Err(e) => return Err(format!("read: {e}")),
        }
    }
    String::from_utf8(line).map_err(|e| format!("utf8: {e}"))
}

/// `SSH-2.0-OpenSSH_8.9p1 Ubuntu-3` → `OpenSSH_8.9p1 Ubuntu-3`
fn format_ident_display(raw: &str) -> Option<String> {
    let s = raw.trim();
    if !s.starts_with("SSH-") {
        return None;
    }
    // SSH-<proto>-<software…>
    let rest = s.strip_prefix("SSH-")?;
    let software = rest.split_once('-').map(|(_, sw)| sw).unwrap_or(rest);
    let software = software.trim();
    if software.is_empty() {
        None
    } else {
        Some(software.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::format_ident_display;

    #[test]
    fn strips_protocol_prefix() {
        assert_eq!(
            format_ident_display("SSH-2.0-OpenSSH_9.6p1 Ubuntu-3ubuntu13.5"),
            Some("OpenSSH_9.6p1 Ubuntu-3ubuntu13.5".into())
        );
        assert_eq!(
            format_ident_display("SSH-2.0-dropbear_2022.83"),
            Some("dropbear_2022.83".into())
        );
        assert_eq!(format_ident_display("not-ssh"), None);
    }
}
