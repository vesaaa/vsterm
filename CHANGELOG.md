# Changelog

All notable releases are published as binaries on
[GitHub Releases](https://github.com/vesaaa/vsterm/releases).

Source development is private; tags that trigger CI live in this repository.
On each `v*` tag, CI also syncs `README.md`, `README.zh-CN.md`, `CHANGELOG.md`,
and `LICENSE` to the public `vesaaa/vsterm` main branch, and copies this file’s
section for that version into the GitHub Release notes.

## [1.2.7] — 2026-09-02

### Added
- **Terminal config** on the host toolbar: toggle gutter **time stamps** and **line numbers** (default on); choices persist in `config.yaml` and cloud preferences sync.
- **Server-tree search** via a filter icon on the left rail.
- **Status bar** shows measured renderer FPS (GPU and software rendering paths).

### Changed
- **SSH connect**: tighter timeouts and clearer failure feedback in the auth dialog.
- **Host import** (WindTerm, Xshell, etc.) runs on a background thread so the UI stays responsive.

### Fixed
- Left sidebar layout: black bar bleed and related tree UX polish.

### 中文
- **新增**：主机工具栏 **终端配置**，可开关 gutter **时间**与**行号**（默认开启），写入 `config.yaml` 并参与云端偏好同步。
- **新增**：左侧会话树 **搜索**（工具栏图标）。
- **新增**：状态栏显示实测渲染 **FPS**（GPU / 软件渲染）。
- **改进**：SSH 连接超时与认证失败提示；主机列表导入改为后台线程。
- **修复**：左侧栏黑边与树交互细节。

## [1.2.6] — 2026-09-01

### Added
- **In-app update checks** now prefer the official CDN manifest at `download.vsterm.com` (Cloudflare R2), with GitHub Releases as fallback; **Download update** opens a platform-matched direct link (zip / dmg / tar.gz).
- **Release CI** syncs stable builds to R2 (`vsterm/latest.json` + per-version binaries); pre-release tags (`v*-beta*`) publish GitHub pre-releases only — no CDN or public `main` sync.

### Changed
- Update-check dialog: **Download update** replaces the old “open download page” primary action; release page remains as a secondary link.

### 中文
- **新增**：检查更新优先走官方 CDN（`download.vsterm.com`），失败回退 GitHub；**下载更新**按本机平台打开直链。
- **新增**：正式版发版 CI 同步产物到 R2；`-beta` 标签仅发 GitHub 预发布，不更新 CDN。
- **改进**：更新对话框主按钮改为「下载更新」。

## [1.2.5] — 2026-09-01

### Added
- **Seven built-in UI themes** under **Options → Theme**: Light, Light-Black, Tokyo Night, Catppuccin Mocha, Nord, Kanagawa Wave, and Everforest Dark. Monitor process/storage rows and gauge labels follow the active theme.
- **Collapsible left rail**: collapse the servers / system-monitor column to a narrow strip via an outline chevron; state persists in `config.yaml` (`left_rail_collapsed`).
- **Credential vault** under **Options → Credential Vault…**: manage reusable SSH passwords and private keys; secrets stay encrypted in `vault.enc`, metadata in `credentials/catalog.yaml`.
- **Session editor**: double-click **Username** to open a credential picker panel beside the dialog; double-click a credential to fill auth fields. Saving a server profile upserts matching credentials when vault save is enabled.

### Changed
- Add/edit server auth form: password and private-key fields align on one row with labels; dialog size stays fixed when switching auth mode.
- Main window opens centered on the work area (accounts for title-bar chrome).

### Fixed
- **Windows**: SSH password login no longer freezes the whole UI after submit.
- **Windows**: local-shell ConPTY sizing — avoid the 2×2 column default and post-output shrink that caused wrap/flash (including 4K high-DPI cell sizing).

### 中文
- **新增**：**选项 → 主题** 内置 7 套主题（Light、Light-Black、Tokyo Night、Catppuccin Mocha、Nord、Kanagawa Wave、Everforest Dark）；监控页进程/磁盘行与仪表标签随主题配色。
- **新增**：左侧**服务器/系统监控列可折叠**（线型三角按钮），状态写入 `config.yaml` 的 `left_rail_collapsed`。
- **新增**：**选项 → 凭证库…**，管理可复用 SSH 密码/私钥；密文存 `vault.enc`，元数据在 `credentials/catalog.yaml`。
- **新增**：添加/编辑服务器时双击**用户名**打开右侧凭证选择面板，双击凭证自动填充；勾选保存时同步写入凭证库。
- **改进**：认证表单标签与输入框同行对齐，切换密码/私钥时对话框尺寸不变；主窗口在工作区内居中启动。
- **修复**：Windows 下 SSH 密码登录提交后不再卡死整个界面；本地 Shell ConPTY 尺寸/高 DPI 下避免异常换行与闪烁。

## [1.2.4] — 2026-08-27

### Added
- Import host lists under **File → Import from Others**: **WindTerm** (`.wind` / `user.sessions`), **Xshell** (`Sessions` / `.xsh`), **FinalShell** (data dir or `conn/`), **MobaXterm** (`MobaXterm.ini` / `.mxtsessions`), **Tabby** (`config.yaml`), **OpenSSH** (`~/.ssh/config`), and **SecureCRT** (`Sessions` / `.ini`). Folder names become groups (two levels). SSH/SFTP only; encrypted passwords are not imported. A private-key *file path* keeps public-key auth.

### 中文
- **新增**：菜单「文件 → 从其他导入」，子项 WindTerm / Xshell / FinalShell / MobaXterm / Tabby / OpenSSH / SecureCRT。只导入 SSH；分组最多两层；密码因对方加密无法导入。私钥文件路径会保留公钥登录。

## [1.2.3] — 2026-08-20

### Changed
- Commands tab menus, new-folder / add-command dialogs, and command-body editing stay responsive: stable chip hit targets, no per-frame command-body clones, and fixed-size editors that do not re-center while typing.

### Added
- Right-click empty space in the command list (below folder chips) to add a command to the current folder.

### 中文
- **改进**：命令页右键、新建文件夹、添加/编辑命令更跟手（固定对话框、避免每帧克隆命令正文）。
- **新增**：命令区空白处右键可添加命令。

## [1.2.2] — 2026-08-12

### Added
- Menu **Check for Updates…** under About: queries public GitHub Releases and reports up-to-date / newer version / failure in a fixed-size dialog.

### Fixed
- Pin production promo signing key `k1` so verified `/v1/promo/channels` drives entitlement list copy (no longer stuck on local legacy fallback without a pin).
- CTA hover text comes from channel `hint` / details / description; omit tooltip when the server sends none.
- Legacy entitlement fallback (unsigned channels) shows only purchase + redeem placeholders — GitHub Star is server-channel only.

### 中文
- **新增**：菜单「检查更新…」，对照公开 GitHub Releases；结果对话框固定尺寸。
- **修复**：钉入生产验签公钥 `k1`，动态权益列表可用；按钮悬停文案跟接口；legacy 兜底仅保留购买/兑换码。

## [1.2.1] — 2026-08-12

### Fixed
- macOS release builds: pin `ip2region` to a resolvable git revision (upstream force-pushed `master` and dropped the old lock SHA) and fetch git deps via CLI to avoid libgit2 SSL failures on runners.

### Docs
- README Standard vs Pro feature table, including Personal Cloud sync crypto notes (Ed25519 device key pair; AES-256-GCM + master password for sync blobs).

### 中文
- **修复**：macOS 发版构建钉死可用的 `ip2region` git 提交，并用 CLI 拉取依赖，避免上游 force-push / SSL 导致 Apple 构建失败。
- **文档**：README 补充标准版 / Pro 对比与云同步加密说明。

## [1.2.0] — 2026-08-12

### Added
- Personal Cloud: signed account session, entitlement channels, redeem, device list, and encrypted vault sync (push/pull with live per-kind progress).
- Local Pro gates: scrollback free 100k / Pro 500k; monkey desk pet requires Pro (dog remains free).
- Desk pets float freely over the whole window (`desk_pet_x`/`desk_pet_y`, `desk_pet_dog_x`/`desk_pet_dog_y`); edge-era configs migrate to near-edge defaults.

### Changed
- GitHub Star OAuth prefers an account picker (`prompt=select_account`) and clears sticky `login=` hints.
- Account prefs: Pro identity strip, dynamic promo actions, email-code rate-limit cooldown with countdown.

### 中文
- **新增**：Personal Cloud（签名会话、权益渠道、兑换、设备列表、加密 vault 同步与分项进度）；本机 Pro 门控（滚动缓冲 / 猴子宠物）；桌宠整窗自由拖放。
- **改进**：GitHub Star OAuth 账号选择；账号页 Pro 身份条与验证码冷却倒计时。

## [1.1.17] — 2026-08-06

### Fixed
- Command History continues after interactive elevation (`sudo -i`, `su`, …). Parent-shell OSC 133 had latched off client Enter marks, while the nested login shell usually has no integration — history stopped. Elevation now re-enables client Enter, and OSC C coalesces with a just-stamped client block to avoid duplicates.

### Docs
- Cloud API contract base URL and response shapes aligned with the public `/sync` gateway (`me.limits`, presigned `required_headers`, download revision metadata).

### 中文
- **修复**：`sudo -i` / `su` 等交互提权后继续记录命令历史（嵌套 shell 常无 OSC 集成，提权后重新启用本机 Enter 打点）。
- **文档**：同步 Cloud API 合同（`/sync` base、`limits`、预签名 `required_headers`、download meta）。

## [1.1.16] — 2026-08-05

### Fixed
- Terminal focus no longer steals arrow / Tab / Escape keys for egui widget navigation after the first press, so shell history recall, Tab completion, and repeated cursor movement stay on the PTY instead of jumping to the History toolbar button.
- Application Cursor Keys (DECCKM) are now encoded from the live emulator mode: bare arrows / Home / End send SS3 (`ESC O A` …) when vi and other fullscreen apps enable `CSI ?1h`, matching xterm terminfo (`kcuu1=\EOA`).

### 中文
- **修复**：终端持焦时锁定方向键 / Tab / Escape，不再被 egui 焦点导航抢走（避免 ↑ 打开「历史记录」）；按模拟器 DECCKM 状态编码应用光标键，vi 等全屏程序中方向键 / Home / End 可用。

## [1.1.15] — 2026-08-04

### Fixed
- Public-key connect dialog no longer starts verifying the moment the first character of an empty username is typed. Auto-verify now requires the username **and** key path to have been prefilled from the saved session; anything typed by hand waits for Verify & Connect. The "Verifying public key…" line only appears when auto-verify will actually run.
- Private key paths are cleaned before use: surrounding quotes (Explorer "Copy as path") and stray whitespace are stripped, and `~\` expands like `~/` on Windows. Existence is checked with `is_file`, so a directory no longer passes as a key.
- "Private key file not found" now names the path that was actually checked, instead of leaving the resolved value invisible.

### Added
- **Browse…** button beside the private key field in both the connect dialog and the session editor, opening a native file picker in the directory already entered.
- Near-miss key suggestions: when a path does not resolve but the same location holds that name with `.key` / `.pem` / `.ppk` / `.openssh`, or a `.pub` was given instead of the private half, the real file is offered as a one-click button. Nothing is substituted silently, and the scan only runs on edit or failed verification.

### Changed
- Saved sessions keep `~` unexpanded, so a session file stays portable when the data directory lives on OneDrive / iCloud / SMB.

### 中文
- **修复**：公钥登录对话框不再在用户名输入第一个字符时就自动验证——仅当用户名与私钥路径均来自已保存会话时才自动发起；私钥路径去除首尾引号与空白，支持 `~\`，存在性检查改用 `is_file`，错误提示带上实际检查的路径。
- **新增**：私钥路径旁「浏览…」按钮（连接对话框与会话编辑器）；路径解析不到时，提示同位置补全 `.key`/`.pem`/`.ppk`/`.openssh` 后的真实文件（或 `.pub` 对应的私钥），点击填入，不做静默替换。
- **改进**：会话保存时保留 `~` 不展开，数据目录放在 OneDrive / iCloud / SMB 时仍可跨机器复用。

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
