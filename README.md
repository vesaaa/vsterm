# VsTerm

**English** | [简体中文](README.zh-CN.md)

**Cross-platform SSH terminal manager** (native Rust)

Drawing on the strengths of **WindTerm**, **Termius**, and **FinalShell**:
professional terminal UX · modern session & credential management · files and ops in one place

> **Source code is private.** This public repository hosts product docs and
> [binary Releases](https://github.com/vesaaa/vsterm/releases) only.
> Development continues in a private source repository.

## Download

Get the latest build from **[Releases](https://github.com/vesaaa/vsterm/releases)**.
Unpack and run. The first launch creates `~/.vsterm/` (including a demo session tree).

Packages are not Microsoft / Apple notarized yet — the OS may warn once.

### Windows (SmartScreen)

1. Unpack and double-click `VsTerm.exe`
2. If Windows protects your PC, open **More info** → **Run anyway**
3. Or: right-click → **Properties** → check **Unblock** → OK

### macOS (blocked / damaged)

1. Unpack the matching chip build: Apple Silicon `vsterm-macos-arm64`, Intel `vsterm-macos-x64`
2. **System Settings → Privacy & Security** → **Open Anyway**
3. If quarantine still blocks:

```bash
xattr -cr /path/to/VsTerm
```

### Linux

```bash
chmod +x VsTerm
./VsTerm
```

## Highlights

- Terminal × SFTP path sync (including elevated / sudo SFTP)
- Session tree, YAML config, encrypted vault
- Host monitor, routes, connections, path traceroute
- ZMODEM (`rz`/`sz`), local shell, multi-tab reconnect
- Desk pets & connect effects

See [CHANGELOG.md](CHANGELOG.md) for version history.

## License & copyright

**All Rights Reserved** (CC BY-NC-ND–style non-commercial / no-derivatives intent).
Full terms: [`LICENSE`](LICENSE).

| Use | Allowed? |
|-----|----------|
| Personal use of official builds | Yes |
| Modify / fork / redistribute builds | No (without written permission) |
| Rebrand / claim IP on derivatives | No |
| Embed in other projects | No |
| Commercial / production / paid delivery | No — buy a commercial license first |

**Commercial licensing:** [vesaazheng@gmail.com](mailto:vesaazheng@gmail.com)

> Third-party libraries and embedded fonts in the binaries remain under their own licenses.
