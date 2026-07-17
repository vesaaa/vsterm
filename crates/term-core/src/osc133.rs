//! OSC 133 (shell integration) + OSC 7 (cwd) byte filter.
//!
//! Sequences are stripped from the PTY stream before alacritty VTE so they
//! never appear on screen; callers receive structured events.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Osc133Kind {
    /// Prompt start.
    PromptStart,
    /// Prompt end / user input begins.
    InputStart,
    /// Command output starts (after Enter).
    OutputStart,
    /// Command finished; optional exit status.
    CommandEnd { exit: Option<i32> },
    /// Property / handshake payload (e.g. `P;VSTERM;READY;<nonce>`).
    Property,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OscEvent {
    Mark(Osc133Kind),
    /// Absolute path from OSC 7 (`file://…`).
    Cwd(String),
}

/// Incremental scanner: feed PTY chunks, get clean bytes + OSC events.
#[derive(Debug, Default)]
pub struct Osc133Filter {
    /// Bytes held while matching an incomplete ESC sequence.
    pending: Vec<u8>,
}

impl Osc133Filter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Process `input`, appending screen bytes to `out` and OSC events to `events`.
    pub fn push(&mut self, input: &[u8], out: &mut Vec<u8>, events: &mut Vec<OscEvent>) {
        if !self.pending.is_empty() {
            let mut combined = std::mem::take(&mut self.pending);
            combined.extend_from_slice(input);
            self.scan(&combined, out, events);
        } else {
            self.scan(input, out, events);
        }
    }

    fn scan(&mut self, data: &[u8], out: &mut Vec<u8>, events: &mut Vec<OscEvent>) {
        let mut i = 0;
        while i < data.len() {
            if data[i] != 0x1b {
                out.push(data[i]);
                i += 1;
                continue;
            }
            if i + 1 >= data.len() {
                self.pending.extend_from_slice(&data[i..]);
                return;
            }
            if data[i + 1] != b']' {
                out.push(data[i]);
                i += 1;
                continue;
            }
            // OSC: ESC ]
            let osc_start = i;
            i += 2;
            let mut body = Vec::new();
            let mut terminated = false;
            while i < data.len() {
                let b = data[i];
                if b == 0x07 {
                    terminated = true;
                    i += 1;
                    break;
                }
                if b == 0x1b {
                    if i + 1 < data.len() && data[i + 1] == b'\\' {
                        terminated = true;
                        i += 2;
                        break;
                    }
                    self.pending.extend_from_slice(&data[osc_start..]);
                    return;
                }
                body.push(b);
                i += 1;
            }
            if !terminated {
                self.pending.extend_from_slice(&data[osc_start..]);
                return;
            }
            if let Some(ev) = parse_osc_body(&body) {
                events.push(ev);
            } else {
                out.extend_from_slice(&data[osc_start..i]);
            }
        }
    }
}

fn parse_osc_body(body: &[u8]) -> Option<OscEvent> {
    let s = std::str::from_utf8(body).ok()?;
    let (code, rest) = match s.split_once(';') {
        Some((c, r)) => (c, r),
        None => (s, ""),
    };
    match code {
        "133" => parse_osc133_kind(rest).map(OscEvent::Mark),
        "7" => parse_osc7_uri(rest).map(OscEvent::Cwd),
        _ => None,
    }
}

fn parse_osc133_kind(rest: &str) -> Option<Osc133Kind> {
    let mut parts = rest.split(';');
    let kind = parts.next()?;
    match kind.chars().next()? {
        'A' => Some(Osc133Kind::PromptStart),
        'B' => Some(Osc133Kind::InputStart),
        'C' => Some(Osc133Kind::OutputStart),
        'D' => {
            let exit = parts.next().and_then(|p| {
                let value = p
                    .strip_prefix("exit=")
                    .or_else(|| p.strip_prefix("status="))
                    .unwrap_or(p);
                value.parse().ok()
            });
            Some(Osc133Kind::CommandEnd { exit })
        }
        // Strip VsTerm handshake / future shell properties from the screen.
        'P' => Some(Osc133Kind::Property),
        _ => None,
    }
}

/// Parse OSC 7 payload: `file://host/path` → `/path`.
fn parse_osc7_uri(uri: &str) -> Option<String> {
    let uri = uri.trim();
    if uri.is_empty() {
        return None;
    }
    let rest = uri.strip_prefix("file://")?;
    // Authority ends at first '/'; path includes that slash.
    let path = match rest.find('/') {
        Some(idx) => &rest[idx..],
        None => return None,
    };
    if path.is_empty() || path.len() > 4096 {
        return None;
    }
    if path.as_bytes().iter().any(|&b| b == 0 || b == b'\n' || b == b'\r') {
        return None;
    }
    let decoded = percent_decode_path(path);
    if decoded.is_empty()
        || decoded.len() > 4096
        || decoded.chars().any(|c| c == '\0' || c == '\n' || c == '\r')
    {
        return None;
    }
    Some(decoded)
}

fn percent_decode_path(path: &str) -> String {
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (from_hex(bytes[i + 1]), from_hex(bytes[i + 2])) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn from_hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_osc133_bel() {
        let mut f = Osc133Filter::new();
        let mut out = Vec::new();
        let mut events = Vec::new();
        let input = b"hello\x1b]133;C\x07world\x1b]133;D;0\x07!";
        f.push(input, &mut out, &mut events);
        assert_eq!(out, b"helloworld!");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0], OscEvent::Mark(Osc133Kind::OutputStart));
        assert_eq!(
            events[1],
            OscEvent::Mark(Osc133Kind::CommandEnd { exit: Some(0) })
        );
    }

    #[test]
    fn strips_osc7_cwd() {
        let mut f = Osc133Filter::new();
        let mut out = Vec::new();
        let mut events = Vec::new();
        let input = b"x\x1b]7;file://localhost/opt\x07y";
        f.push(input, &mut out, &mut events);
        assert_eq!(out, b"xy");
        assert_eq!(events, vec![OscEvent::Cwd("/opt".into())]);
    }

    #[test]
    fn osc7_file_triple_slash() {
        assert_eq!(
            parse_osc7_uri("file:///tmp/foo"),
            Some("/tmp/foo".into())
        );
    }

    #[test]
    fn osc7_percent_decode() {
        assert_eq!(
            parse_osc7_uri("file://host/home/a%20b"),
            Some("/home/a b".into())
        );
    }

    #[test]
    fn passes_other_osc() {
        let mut f = Osc133Filter::new();
        let mut out = Vec::new();
        let mut events = Vec::new();
        let input = b"\x1b]0;title\x07ok";
        f.push(input, &mut out, &mut events);
        assert_eq!(out, input);
        assert!(events.is_empty());
    }

    #[test]
    fn split_across_chunks() {
        let mut f = Osc133Filter::new();
        let mut out = Vec::new();
        let mut events = Vec::new();
        f.push(b"x\x1b]13", &mut out, &mut events);
        f.push(b"3;A\x07y", &mut out, &mut events);
        assert_eq!(out, b"xy");
        assert_eq!(events, vec![OscEvent::Mark(Osc133Kind::PromptStart)]);
    }

    #[test]
    fn parses_exit_named_status() {
        assert_eq!(
            parse_osc133_kind("D;exit=7"),
            Some(Osc133Kind::CommandEnd { exit: Some(7) })
        );
    }

    #[test]
    fn strips_ready_property() {
        let mut f = Osc133Filter::new();
        let mut out = Vec::new();
        let mut events = Vec::new();
        let input = b"a\x1b]133;P;VSTERM;READY;abc\x07b";
        f.push(input, &mut out, &mut events);
        assert_eq!(out, b"ab");
        assert_eq!(events, vec![OscEvent::Mark(Osc133Kind::Property)]);
    }

    #[test]
    fn rejects_osc7_control_chars() {
        assert_eq!(parse_osc7_uri("file://host/tmp/\nfoo"), None);
    }
}
