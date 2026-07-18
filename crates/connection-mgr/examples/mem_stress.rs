//! Headless RSS / scrollback stress for VsTerm connection + terminal paths.
//!
//! Usage (with local sshd on 127.0.0.1:2222 + pubkey auth):
//!   cargo run -p connection-mgr --example mem_stress --release
//!
//! Env:
//!   VSTERM_STRESS_HOST   default 127.0.0.1
//!   VSTERM_STRESS_PORT   default 2222
//!   VSTERM_STRESS_USER   default $USER
//!   VSTERM_STRESS_KEY    default ~/.ssh/id_ed25519
//!   VSTERM_STRESS_CONNS  default 4

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use connection_mgr::ConnectionManager;
use session_tree::{AuthConfig, SessionConfig};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use term_core::TerminalHandle;

fn rss_kb() -> u64 {
    let Ok(s) = std::fs::read_to_string("/proc/self/status") else {
        return 0;
    };
    for line in s.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            let num: String = rest.chars().filter(|c| c.is_ascii_digit()).collect();
            return num.parse().unwrap_or(0);
        }
    }
    0
}

fn fmt_mb(kb: u64) -> String {
    format!("{:.1} MiB", kb as f64 / 1024.0)
}

fn delta(a: u64, b: u64) -> String {
    let d = b as i64 - a as i64;
    format!("{:+.1} MiB", d as f64 / 1024.0)
}

fn ui_runtime() -> tokio::runtime::Runtime {
    // Mirror the app UI thread: a lightweight runtime that only awaits
    // `spawn_blocking` into the process-global russh runtime.
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("ui runtime")
}

fn session_config(i: usize) -> SessionConfig {
    let host = std::env::var("VSTERM_STRESS_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let port: u16 = std::env::var("VSTERM_STRESS_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2222);
    let user = std::env::var("VSTERM_STRESS_USER").unwrap_or_else(|_| {
        std::env::var("USER").unwrap_or_else(|_| "ubuntu".into())
    });
    let key = std::env::var("VSTERM_STRESS_KEY")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".ssh/id_ed25519")
        });
    SessionConfig {
        id: format!("stress-{i}"),
        name: format!("stress-{i}"),
        host,
        port,
        username: user,
        auth: AuthConfig::Publickey {
            private_key_path: key,
            passphrase_ref: None,
        },
        color_tag: None,
        icon: None,
        term_type: "xterm-256color".into(),
        shell_integration: false,
    }
}

fn feed_lines(term: &TerminalHandle, n: usize) {
    let mut chunk = Vec::with_capacity(96 * 256);
    for i in 0..n {
        let line = format!("LINE-{i:06} {}\n", "x".repeat(64));
        chunk.extend_from_slice(line.as_bytes());
        if chunk.len() >= 64 * 1024 {
            term.advance_bytes(&chunk);
            chunk.clear();
        }
    }
    if !chunk.is_empty() {
        term.advance_bytes(&chunk);
    }
}

fn main() {
    println!("=== VsTerm mem_stress (allocator=mimalloc) ===");
    println!("pid={}  baseline RSS={}", std::process::id(), fmt_mb(rss_kb()));

    let n_conns: usize = std::env::var("VSTERM_STRESS_CONNS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(4);

    let rt = ui_runtime();
    let mgr = ConnectionManager::new();
    let before_open = rss_kb();
    println!("\n[1] Open {n_conns} SSH sessions");
    let mut ids = Vec::new();
    for i in 0..n_conns {
        let cfg = session_config(i);
        match rt.block_on(mgr.open_session(&cfg, None)) {
            Ok(id) => {
                // SFTP like the files panel — must not call block_on on the UI
                // runtime thread; run on a worker (same as app SFTP threads).
                if let Some(remote) = mgr.active_remote() {
                    let remote2 = remote.clone();
                    let list = thread::spawn(move || remote2.list_dir("/")).join();
                    match list {
                        Ok(Ok(entries)) => println!(
                            "  conn#{i} ok  sftp list=/ → {} entries  RSS={}",
                            entries.len(),
                            fmt_mb(rss_kb())
                        ),
                        Ok(Err(e)) => println!(
                            "  conn#{i} ok  sftp list failed: {e}  RSS={}",
                            fmt_mb(rss_kb())
                        ),
                        Err(e) => println!("  conn#{i} ok  sftp join err: {e:?}"),
                    }
                } else {
                    println!("  conn#{i} ok  (no remote) RSS={}", fmt_mb(rss_kb()));
                }
                ids.push(id);
            }
            Err(e) => {
                eprintln!("  conn#{i} FAILED: {e}");
                eprintln!("  (need sshd on VSTERM_STRESS_HOST:PORT with pubkey auth)");
                std::process::exit(2);
            }
        }
    }
    let after_open = rss_kb();
    let per = if n_conns > 0 {
        (after_open as i64 - before_open as i64) as f64 / n_conns as f64 / 1024.0
    } else {
        0.0
    };
    println!(
        "  after open: {}  ({})  ≈ {:.1} MiB/conn",
        fmt_mb(after_open),
        delta(before_open, after_open),
        per
    );

    println!("  closing all tabs…");
    for id in ids {
        mgr.close(id);
    }
    for wait in [1u64, 2, 4] {
        thread::sleep(Duration::from_secs(wait));
        println!("  +{}s after close: RSS={}", wait, fmt_mb(rss_kb()));
    }
    let after_close = rss_kb();
    println!(
        "  residual vs pre-open: {}  (expect small; not necessarily back to baseline)",
        delta(before_open, after_close)
    );
    println!("  reclaimed vs peak: {}", delta(after_open, after_close));

    // Keep mgr alive until after reclaim waits, then drop off the UI runtime
    // so RusshRemoteExec::drop's block_on is not nested.
    drop(mgr);
    thread::sleep(Duration::from_millis(500));
    unsafe {
        libmimalloc_sys::mi_collect(true);
    }
    println!("  after drop manager: RSS={}", fmt_mb(rss_kb()));

    let rounds: usize = std::env::var("VSTERM_STRESS_CYCLES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    if rounds > 0 {
        println!("\n[1b] Open/close cycles x{rounds} (conns={n_conns})");
        let mut prev = rss_kb();
        for r in 1..=rounds {
            let mgr = ConnectionManager::new();
            let mut ids = Vec::new();
            for i in 0..n_conns {
                let cfg = session_config(i);
                let id = rt.block_on(mgr.open_session(&cfg, None)).expect("open");
                if let Some(remote) = mgr.active_remote() {
                    let remote2 = remote.clone();
                    let _ = thread::spawn(move || remote2.list_dir("/")).join();
                }
                ids.push(id);
            }
            let peak = rss_kb();
            for id in ids {
                mgr.close(id);
            }
            thread::sleep(Duration::from_secs(4));
            drop(mgr);
            thread::sleep(Duration::from_millis(500));
            unsafe { libmimalloc_sys::mi_collect(true); }
            let now = rss_kb();
            println!(
                "  cycle {r}: peak={}  after={}  Δvs prev_after={}  Δvs start={}",
                fmt_mb(peak),
                fmt_mb(now),
                delta(prev, now),
                delta(before_open, now)
            );
            prev = now;
        }
    }

    println!("\n[2] Scrollback feed (fresh TerminalHandle per size, 80x24)");
    let empty_base = {
        let t = TerminalHandle::new(80, 24);
        let r = rss_kb();
        drop(t);
        unsafe {
            libmimalloc_sys::mi_collect(true);
        }
        println!("  empty terminal RSS≈{}", fmt_mb(r));
        r
    };

    for n in [5_000usize, 10_000, 20_000, 50_000] {
        let before = rss_kb();
        let term = TerminalHandle::new(80, 24);
        feed_lines(&term, n);
        let snap = term.snapshot();
        let after = rss_kb();
        println!(
            "  {n:>6} lines → history={}  RSS={}  (alloc {})",
            snap.history_size,
            fmt_mb(after),
            delta(before, after)
        );

        if n == 50_000 {
            let peak = after;
            println!("  clear screen (ESC[H ESC[2J) on 50k buffer…");
            term.advance_bytes(b"\x1b[H\x1b[2J");
            thread::sleep(Duration::from_millis(200));
            unsafe {
                libmimalloc_sys::mi_collect(true);
            }
            let after_cls = rss_kb();
            let snap = term.snapshot();
            println!(
                "  after cls: history={}  RSS={}  vs peak={}",
                snap.history_size,
                fmt_mb(after_cls),
                delta(peak, after_cls)
            );

            println!("  erase scrollback (ESC[3J)…");
            term.advance_bytes(b"\x1b[3J");
            thread::sleep(Duration::from_millis(200));
            unsafe {
                libmimalloc_sys::mi_collect(true);
            }
            let after_erase = rss_kb();
            let snap = term.snapshot();
            println!(
                "  after ESC[3J: history={}  RSS={}  vs peak={}",
                snap.history_size,
                fmt_mb(after_erase),
                delta(peak, after_erase)
            );

            drop(term);
            thread::sleep(Duration::from_millis(300));
            unsafe {
                libmimalloc_sys::mi_collect(true);
            }
            println!(
                "  after drop 50k terminal: RSS={}  vs empty_base={}",
                fmt_mb(rss_kb()),
                delta(empty_base, rss_kb())
            );
        } else {
            drop(term);
            unsafe {
                libmimalloc_sys::mi_collect(true);
            }
        }
    }

    println!("\n=== done ===");
}
