//! OSC 133 (Final Term / shell integration) byte filter.
//!
//! Sequences are stripped from the PTY stream before alacritty VTE so they
//! never appear on screen; callers receive structured mark events.

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
}

/// Incremental scanner: feed PTY chunks, get clean bytes + mark events.
#[derive(Debug, Default)]
pub struct Osc133Filter {
    /// Bytes held while matching an incomplete ESC sequence.
    pending: Vec<u8>,
}

impl Osc133Filter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Process `input`, appending screen bytes to `out` and OSC 133 marks to `marks`.
    pub fn push(&mut self, input: &[u8], out: &mut Vec<u8>, marks: &mut Vec<Osc133Kind>) {
        if !self.pending.is_empty() {
            let mut combined = std::mem::take(&mut self.pending);
            combined.extend_from_slice(input);
            self.scan(&combined, out, marks);
        } else {
            self.scan(input, out, marks);
        }
    }

    fn scan(&mut self, data: &[u8], out: &mut Vec<u8>, marks: &mut Vec<Osc133Kind>) {
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
            if let Some(kind) = parse_osc133_body(&body) {
                marks.push(kind);
            } else {
                out.extend_from_slice(&data[osc_start..i]);
            }
        }
    }
}

fn parse_osc133_body(body: &[u8]) -> Option<Osc133Kind> {
    let s = std::str::from_utf8(body).ok()?;
    let mut parts = s.split(';');
    let code = parts.next()?;
    if code != "133" {
        return None;
    }
    let kind = parts.next()?;
    match kind.chars().next()? {
        'A' => Some(Osc133Kind::PromptStart),
        'B' => Some(Osc133Kind::InputStart),
        'C' => Some(Osc133Kind::OutputStart),
        'D' => {
            let exit = parts.next().and_then(|p| {
                let p = p.split('=').next().unwrap_or(p);
                p.parse().ok()
            });
            Some(Osc133Kind::CommandEnd { exit })
        }
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
        let mut marks = Vec::new();
        let input = b"hello\x1b]133;C\x07world\x1b]133;D;0\x07!";
        f.push(input, &mut out, &mut marks);
        assert_eq!(out, b"helloworld!");
        assert_eq!(marks.len(), 2);
        assert_eq!(marks[0], Osc133Kind::OutputStart);
        assert_eq!(marks[1], Osc133Kind::CommandEnd { exit: Some(0) });
    }

    #[test]
    fn passes_other_osc() {
        let mut f = Osc133Filter::new();
        let mut out = Vec::new();
        let mut marks = Vec::new();
        let input = b"\x1b]0;title\x07ok";
        f.push(input, &mut out, &mut marks);
        assert_eq!(out, input);
        assert!(marks.is_empty());
    }

    #[test]
    fn split_across_chunks() {
        let mut f = Osc133Filter::new();
        let mut out = Vec::new();
        let mut marks = Vec::new();
        f.push(b"x\x1b]13", &mut out, &mut marks);
        f.push(b"3;A\x07y", &mut out, &mut marks);
        assert_eq!(out, b"xy");
        assert_eq!(marks, vec![Osc133Kind::PromptStart]);
    }
}
