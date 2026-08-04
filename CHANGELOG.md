# Changelog

All notable releases are published as binaries on
[GitHub Releases](https://github.com/vesaaa/vsterm/releases).

Source development is private; tags that trigger CI live in this repository.
On each `v*` tag, CI also syncs `README.md`, `README.zh-CN.md`, `CHANGELOG.md`,
and `LICENSE` to the public `vesaaa/vsterm` main branch, and copies this file’s
section for that version into the GitHub Release notes.

## [1.1.14] — 2026-08-04

### Fixed
- Path Trace: IPv6 targets now use family-correct tooling (`-4`/`-6`, `traceroute6` / `tracert -6`, Auto / IPv4 / IPv6 mode); dual-stack hosts no longer silently produce empty hop lists for IPv6.
- IP Quality: public IPv6 probe pins address family (`curl -6` / remote script equivalents) with fallbacks so dual-stack endpoints that advertise A records are not misread as IPv4-only.

### Changed
- Idle password auth dialog no longer forces a continuous redraw loop; software renderers keep a solid caret to avoid unnecessary whole-window paints.
- Terminal PTY output and Monitor / System Info snapshot updates repaint at the renderer cadence (or when data lands) instead of uncapped or fixed polling, cutting high CPU on software adapters under load or idle dialogs.

### 中文
- **修复**：路径追踪 IPv6 目标使用对应族探测与 Auto/IPv4/IPv6 模式；IP 质量本机/远端公网 IPv6 探测正确绑定 IPv6，避免被误判为无 IPv6。
- **改进**：空闲认证弹窗与终端输出重绘按渲染节奏合并，显著降低软件渲染下的 CPU 占用。

## [1.1.13] — 2026-07-28

### Changed
- Net connections: remote location uses the same cascade as path trace (ipinfo → ibxq → ip2region), filled asynchronously so the table stays responsive; auto-refresh every 5s.
- Net connections charts: hover shows a point and Y value (no vertical crosshair); X-axis shows up to three `mm:ss` sample times.
- SSH: enable TCP keepalive (idle ~60s, probe every 10s) alongside the existing 30s application-layer keepalive.
- Geo/location lookups skip CGNAT `100.64.0.0/10` (e.g. Tailscale) in addition to RFC1918 private ranges.

### 中文
- **改进**：网络连接归属地与路径追踪同级联查询，后台补全；自动刷新改为 5 秒；图表悬停显示折线点与 Y 值，X 轴最多三个分:秒时间点。
- **改进**：SSH 增加 TCP keepalive；归属地查询跳过 CGNAT `100.64/10`。

## [1.1.12] — 2026-07-27

### Changed
- Routes diagram: one chain per policy table (prefer that table’s default), show `ip rule` source (e.g. `from …`) and metrics on exit nodes; remove the unused click-detail text under a selected spoke.

### 中文
- **改进**：图形路由按策略表合并链路、展示规则来源与 metric；去掉点击链路后下方无用的详情文本。

## [1.1.11] — 2026-07-27

### Added
- Routes panel **diagram** view: hub-and-spoke graph for default/policy routes, Linux TPROXY / mark chains (e.g. fwmark → table → lo → TPROXY port / listener), and LAN gateway + NAT detection from `ip rule` / `iptables` / `ss`/`netstat`/`/proc`.
- Terminal selection persists across scroll; **Shift+click** extends the selection across scrollback.

### Fixed
- Terminal: Ctrl+C copy no longer freezes the UI (clipboard write was re-entering the egui input lock).

### 中文
- **新增**：路由面板「图形」视图（默认/策略路由、TPROXY/mark 链路、局域网网关与 NAT）；终端选区滚动后保留，可用 Shift+点击扩展。
- **修复**：终端 Ctrl+C 复制不再卡死。

## [1.1.10] — 2026-07-27

### Fixed
- Windows: opening IP Quality no longer flashes two console windows when probing local public IPv4/IPv6 via `curl` (`CREATE_NO_WINDOW`).

### 中文
- **修复**：Windows 上打开 IP 质量检测时，本地探测公网 IPv4/IPv6 不再弹出两个控制台黑框。

## [1.1.9] — 2026-07-27

### Changed
- IP Quality: public IPv4/IPv6 chips reflect the **active host** — remote SSH runs `curl`/`wget` on the server; Local Shell probes this machine; disconnected hosts show an unavailable state.
- Fraud score bar and risk badges follow Scamalytics `scamalytics_risk` (e.g. score 25 → medium/yellow), not equal 0–100 thirds.

### 中文
- **改进**：IP 质量检测的公网 IPv4/IPv6 标签随当前连接变化（SSH 远端在服务器上探测、本地 Shell 探测本机、断开时不可用）。
- **改进**：欺诈分进度条与风险色块对齐 Scamalytics 风险等级。

## [1.1.8] — 2026-07-27

### Added
- Toolbox **IP Quality Check**: queries `api.ibxq.com` fraud data (risk score bar, operator/line, location, IP type, cloud vendor identity, blacklists, bots, proxies).
- Local public IPv4/IPv6 chips next to Check (via `curl` / `ipv4.ip.sb` · `ipv6.ip.sb`); click to fill the target field.
- Path Trace is toolbox-only (no longer a top tab); Toolbox also keeps Traceroute.

### Changed
- IP Quality layout polish: paired ISP/Org and postcode/timezone rows; IP-type chips (yes-only, centered); vendor / blacklist chip grids.

### 中文
- **新增**：工具箱「IP质量检测」（欺诈分、运营商、地理位置、IP 类型、大厂身份、外部黑名单、Bot、代理）；本机公网 IPv4/IPv6 标签可点击填入；路径追踪仅保留工具箱入口。
- **改进**：IP 质量面板排版（双字段对齐、类型色块居中、黑名单方框+尾部色块等）。

## [1.1.7] — 2026-07-27

### Changed
- Path Trace default target is `8.8.4.4` (was `www.baidu.com`).
- CI: public `vesaaa/vsterm` Release and docs sync only after **all** platform builds succeed (draft staging; incomplete builds are aborted).

### 中文
- **改进**：路径追踪默认目标改为 `8.8.4.4`；Release CI 等全部平台构建成功后再公开发布与同步文档，避免缺包。

## [1.1.6] — 2026-07-26

### Added
- Preferences: when setting a master password for the first time, **Screen lock** is checked by default (and persisted after a successful set).
- Preferences → Data directory: help text notes that the default folder is `.vsterm`; the path field supports typing/paste and a right-click Copy/Paste menu.
- Fresh installs default the desk pet to the **bottom-edge dog**.

### Changed
- Session tree folders use a macOS-style `>` disclosure chevron (rotates when open).
- Left-click a folder icon/name expands or collapses children; right-click still opens the context menu (add server, rename, …).

### Fixed
- Folder name hover no longer shows the text I-beam cursor (arrow cursor instead).

### 中文
- **新增**：首次设置主密码时默认勾选屏幕锁；数据目录说明标明默认 `.vsterm`，路径可粘贴；首次安装默认启用底部小狗桌面宠物。
- **改进**：文件夹折叠箭头改为 macOS 风格 `>`；左键点名称展开/收起，右键可添加服务器等。
- **修复**：服务器列表里文件夹名悬停光标恢复为箭头。

## [1.1.5] — 2026-07-26

- CI: run release asset upload with bash on all runners (fixes Windows PowerShell failing on `set -euo pipefail`); bump actions/checkout to v5.

## [1.1.4] — 2026-07-26

- CI: tag releases upload assets directly to the public Release (skip Actions artifacts; avoids storage-quota failures).

## [1.1.3] — 2026-07-26

- Cut idle CPU: vsync on real GPUs (WARP still AutoNoVsync); stop tooltip popups from scheduling 60 fps redraws.
- macOS Dock / Launchpad icon: 10% transparent margin; runtime Dock glyph uses the same padded asset.
- macOS releases ship as drag-install DMGs; `tools/` (ip2region) lives inside `VsTerm.app/Contents/Resources`.

## [1.1.2] — 2026-07-24

- Verify private CI publishes Release assets (and docs) to public `vesaaa/vsterm`.

## [1.1.0] — 2026-07-24

- Fix terminal drag selection starting late (anchor at press origin so fast drags no longer skip the first characters).

## [1.0.49] — 2026-07-24

- SSH tunnels: one shared forward set per host across tabs; UI polish (icons, SOCKS5 defaults, panel height).

## [1.0.48] — 2026-07-23

- Six-locale UI, release polish, bilingual README.

## [1.0.47] — 2026-07-22

- Dual geo providers for path trace; network-connections filter refresh.

## [1.0.46] — 2026-07-21

- Commands panel: folders, icons, multi-line paste.

## Earlier

See [Releases](https://github.com/vesaaa/vsterm/releases) for the full history (`v1.0.0` … `v1.0.45`).
