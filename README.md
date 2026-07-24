# VsTerm

**English** | [简体中文](README.zh-CN.md)

<p align="center">
  <img src="assets/branding/logo.png" alt="VsTerm" width="160" height="160">
</p>

<p align="center">
  <strong>Cross-platform SSH terminal manager</strong> (native Rust)
</p>

<p align="center">
  Drawing on the strengths of <strong>WindTerm</strong>, <strong>Termius</strong>, and <strong>FinalShell</strong>:<br>
  professional terminal UX · modern session &amp; credential management · files and ops in one place
</p>

- GUI: `egui` + `eframe` (`wgpu`: Windows DX12 / macOS Metal / Linux Vulkan)
- Terminal: `alacritty_terminal`
- SSH: built-in `russh` (PTY, remote commands, and SFTP share one authenticated session)
- Config: YAML; credentials: OS keyring + encrypted vault

## Highlights

### Terminal × SFTP path sync, with elevation

The file pane and the shell stay aligned:

- **Files → terminal**: one-click `cd` into the remote folder you are browsing
- **Terminal → files**: OSC 7 cwd reporting keeps the file pane following directory changes
- **sudo / elevated SFTP**: after `sudo -i` / `su` in the terminal, transfers can use the same identity; or elevate with a secure password dialog
- Real SFTP: list, navigate, file/folder upload &amp; download, progress and queue; drag-and-drop, create / rename / delete

### Best of breed, with its own path

| Inspired by | In VsTerm |
|-------------|-----------|
| **WindTerm** | Command-block fold gutter, timestamps &amp; line numbers, semantic highlighting, deep scrollback |
| **Termius** | Session tree &amp; folders, portable YAML config, encrypted vault / master password |
| **FinalShell** | Bottom file manager, host ops toolbox, visual CPU / network / storage monitors |

Also: quick connect (`user@host` / `ssh://…`), multi-tab &amp; reconnect, local shell, ZMODEM (`rz`/`sz`), system monitor and system-info panels.

### Desk pets &amp; connect effects — ops with a pulse

- **Desk pets**: monkey (right edge) / dog (bottom edge), draggable; reacts to typing, Enter, and host connect
- **Connect effects**: trail inhale / shatter rebuild; tab accent sweep after connect
- Falls back to a lower frame cadence when no hardware GPU is available (e.g. some RDP / WARP setups)

### Network tools

Built-in ops panels for local and remote hosts:

- **Routes**: full IPv4 / IPv6 tables and `ip rule` policy rules — see multi-WAN / policy routing without guessing
- **Connections**: TCP / UDP overview; host connection details; router NAT forward sessions with filters
- **Path trace**: per-hop RTT / loss with ASN, org, location, and coordinates for line quality checks

## First run

Download a build from [Releases](https://github.com/vesaaa/vsterm/releases), unpack, and run. The first launch creates `~/.vsterm/` (including a demo session tree).

Packages are not Microsoft / Apple notarized yet — the OS may warn once; use the steps below.

### Windows (SmartScreen)

1. Unpack and double-click `VsTerm.exe`
2. If Windows protects your PC, open **More info**
3. Choose **Run anyway**

Or: right-click `VsTerm.exe` → **Properties** → check **Unblock** → OK, then open again.

### macOS (blocked / damaged)

1. Unpack the matching chip build: Apple Silicon `vsterm-macos-arm64`, Intel `vsterm-macos-x64`
2. If Gatekeeper blocks it: **System Settings → Privacy &amp; Security** → **Open Anyway**
3. If quarantine still blocks it:

```bash
xattr -cr /path/to/VsTerm
```

### Linux

```bash
chmod +x VsTerm
./VsTerm
```

## Font licenses

Embedded fonts are SIL OFL 1.1; see `assets/fonts/`.

## License &amp; copyright

Source is **All Rights Reserved**, guided by **CC BY-NC-ND 4.0**-style non-commercial / no-derivatives intent. Full terms: [`LICENSE`](LICENSE).

**Copyright holder:** vesaa

| Use | Allowed? |
|-----|----------|
| Personal study / local build | Yes |
| Modify / fork / redistribute builds | No (without written permission) |
| Rebrand / claim IP on derivatives | No |
| Embed in other projects (incl. closed source) | No |
| Commercial / production / paid delivery | No — buy a commercial license first |

**Commercial licensing:** [vesaazheng@gmail.com](mailto:vesaazheng@gmail.com)

> Third-party libraries and embedded fonts remain under their own licenses.
