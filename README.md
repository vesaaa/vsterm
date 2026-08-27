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

### What stands out in v1.1.13

- **Terminal + files stay in one flow**: shell tabs, bottom SFTP pane, sudo/elevated SFTP, and ZMODEM transfers are integrated into the same SSH workflow.
- **Ops panels are first-class, not side utilities**: routes, connections, path trace, IP quality, system info, and live host metrics are built into the app.
- **Progress is visible**: both **SFTP** and **ZMODEM (`rz` / `sz`)** transfers expose progress / queue state instead of disappearing into a blocking dialog or a blind background task.
- **Topology and line-quality tooling is richer than a plain terminal**: routing diagrams, per-hop geo/ASN enrichment, fraud / datacenter / blacklist checks, and connection charts are available without leaving the session.
- **Native desktop app**: no Electron; Rust + `wgpu`, with software-render fallback for VM / RDP-style environments.

### Screenshots

#### Terminal workspace

<img src="assets/screenshots/overview_terminal.webp" alt="VsTerm terminal overview with session tree, terminal, and bottom pane" />

#### Route diagram / policy topology

<img src="assets/screenshots/routes_diagram.webp" alt="VsTerm routes diagram with policy topology" />

#### IP quality check

<img src="assets/screenshots/ip_quality.webp" alt="VsTerm IP quality panel with risk, org, location, and blacklist results" />

#### Path trace with geo / ASN enrichment

<img src="assets/screenshots/path_trace_geo.webp" alt="VsTerm path trace showing RTT, loss, org, ASN, location, and coordinates" />

### Feature comparison across mainstream SSH terminal tools

Legend: `✅` built in and surfaced as a first-class workflow, `◐` supported in a narrower / companion-tool / plugin-style way, `✗` not a headline built-in capability in the usual product workflow.

| Capability | VsTerm | WindTerm | Termius | FinalShell | MobaXterm | SecureCRT | Xshell | Tabby |
|------------|--------|----------|---------|------------|-----------|-----------|--------|-------|
| Implementation language | Rust | C/C++ | Electron | Java | C++ | C++ | C++ | Electron |
| Max terminal scrollback lines | 100k / 500k (Pro) | unlimited | - | - | 360,000 | 128,000 | ~2.1B | 25,000 |
| Command-block folding / outline in terminal output | ✅ | ✅ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |
| Integrated SFTP pane / remote file manager | ✅ | ✅ | ✅ | ✅ | ✅ | ◐ | ◐ | ◐ |
| SFTP transfer progress / queue visibility | ✅ | ◐ | ◐ | ✅ | ◐ | ◐ | ◐ | ◐ |
| ZMODEM (`rz` / `sz`) built in | ✅ | ✅ | ✗ | ✅ | ◐ | ✅ | ✅ | ✅ |
| ZMODEM progress surfaced in the app | ✅ | ✅ | ✗ | ◐ | ◐ | ◐ | ◐ | ◐ |
| Terminal ↔ file-pane path sync | ✅ | ✗ | ◐ | ◐ | ✗ | ✗ | ✗ | ✗ |
| Elevated SFTP that can follow `sudo -i` / `su` | ✅ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |
| Route diagram / policy-routing topology | ✅ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |
| Path trace with geo / ASN enrichment | ✅ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |
| Built-in IP quality / reputation checks | ✅ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |
| CPU / memory / storage graphical monitor | ✅ | ◐ | ✗ | ✅ | ✗ | ✗ | ✗ | ✗ |
| Connection / socket monitoring panel | ✅ | ✗ | ✗ | ◐ | ✗ | ✗ | ✗ | ✗ |
| Connect effects / motion polish | ✅ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |
| Desk pet | ✅ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |

Notes:

- The table is intentionally strict: it favors **built-in, documented, end-user-visible workflows** over “possible via shell commands” or “can be scripted externally”.
- Some commercial tools split file-transfer UX into a companion app or a separate tab, which is why several cells are `◐` instead of `✅`.
- Scrollback figures use publicly documented defaults / maxima where available (`-` = not clearly documented). VsTerm grows dynamically: **100,000** lines on Standard, **500,000** on Pro.

### Standard vs Pro

VsTerm runs fully offline for SSH work. **Personal Cloud** (account login, entitlement, and encrypted sync) is optional. **Pro** unlocks after a binding-checked cloud entitlement (e.g. GitHub Star promo or redeem).

| Capability | Standard | Pro |
|------------|----------|-----|
| Local SSH sessions, SFTP, ZMODEM, ops panels | ✅ | ✅ |
| Encrypted local credential vault | ✅ | ✅ |
| Max terminal scrollback | 100,000 lines | 500,000 lines |
| Desk pet — dog (free-floating) | ✅ | ✅ |
| Desk pet — monkey (free-floating) | ✗ | ✅ |
| Personal Cloud account / devices | ✅ | ✅ |
| Cloud sync (sessions, commands, layouts, preferences, credentials) | ✗ | ✅ |
| Future enhancements | Not supported | Continuously supported |

#### Cloud sync &amp; data security

Uploads are **client-side encrypted**. The server stores ciphertext only and cannot read your sessions, commands, layouts, preferences, or vault contents.

| Layer | Algorithm / model |
|-------|-------------------|
| Device identity (local key pair) | **Ed25519** — private key stays in the OS keyring; never uploaded |
| Sync object encryption | **AES-256-GCM**, key derived from your **master password** (Argon2id → VSC2 blob) |
| Credential vault inside sync | Already encrypted locally; sync wraps the sealed `vault.enc`, not plaintext passwords |

Without your master password (and the derived sync key), cloud blobs are not decryptable — including by VsTerm operators.

### Desk pets &amp; connect effects — ops with a pulse

- **Desk pets**: dog (Standard) / monkey (Pro); free-drag anywhere in the window; reacts to typing, Enter, and host connect
- **Connect effects**: trail inhale / shatter rebuild; tab accent sweep after connect
- Falls back to a lower frame cadence when no hardware GPU is available (e.g. some RDP / WARP setups)

## First run

Download a build from [Releases](https://github.com/vesaaa/vsterm/releases), unpack, and run. The first launch creates `~/.vsterm/` (including a demo session tree).

To bring over a host list: **File → Import from Others**, then **WindTerm** (`.wind` / `profiles/`), **Xshell** (`Sessions` / `.xsh`), **FinalShell** (data dir or `conn/`), **MobaXterm** (`MobaXterm.ini` / `.mxtsessions`), **Tabby** (`config.yaml`), **OpenSSH** (`~/.ssh/config`), or **SecureCRT** (`Sessions` / `.ini`). SSH hosts and groups come across (two folder levels). Encrypted passwords are not copied — you enter them on first connect. A private-key *file path* keeps public-key auth; named keys stored inside Xshell / FinalShell are not resolved.

Packages are not Microsoft / Apple notarized yet — the OS may warn once; use the steps below.

### Windows (SmartScreen)

1. Unpack and double-click `VsTerm.exe`
2. If Windows protects your PC, open **More info**
3. Choose **Run anyway**

Or: right-click `VsTerm.exe` → **Properties** → check **Unblock** → OK, then open again.

### macOS (blocked / damaged)

1. Download the matching chip DMG: Apple Silicon `vsterm-macos-arm64.dmg`, Intel `vsterm-macos-x64.dmg`
2. Open the DMG and drag **VsTerm** into **Applications** (runtime data such as GeoIP is inside the `.app` — no separate `tools/` folder)
3. If Gatekeeper blocks it: **System Settings → Privacy &amp; Security** → **Open Anyway**
4. If quarantine still blocks it:

```bash
xattr -cr /Applications/VsTerm.app
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
