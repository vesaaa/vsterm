# Changelog

All notable releases are published as binaries on
[GitHub Releases](https://github.com/vesaaa/vsterm/releases).

Source development is private; tags that trigger CI live in this repository.
On each `v*` tag, CI also syncs `README.md`, `README.zh-CN.md`, `CHANGELOG.md`,
and `LICENSE` to the public `vesaaa/vsterm` main branch, and copies this file’s
section for that version into the GitHub Release notes.

## [Unreleased]

## [1.3.7] — 2026-10-07

### Fixed
- **macOS Keychain prompts after an update**: the first launch of a new version asked for the Keychain password several times, once per stored item, even after "Always Allow". The app is now signed with a fixed certificate, so "Always Allow" carries over to later updates. Launch also no longer reads three of those items: the command-history key comes from its local key file, the Pro trial uses a new item, and the cloud device key and sync key load only when needed. Updating to this version can still ask once if "Remember unlock" is on, plus twice more if you are signed in to a cloud account. Later updates should not ask again.

### 中文
- **修复**：macOS 升级后第一次打开时会连续弹出好几次钥匙串密码框（每个存储的条目弹一次），点了「始终允许」下次升级还会再弹。现在程序改用固定证书签名，「始终允许」在以后的升级中会一直有效。启动时也不再读取其中三个条目：命令历史的密钥改从本地密钥文件读取，Pro 试用改用新条目，云设备密钥和同步密钥改为用到时才读取。升级到本版本时，如果开启了「记住解锁」仍可能弹 1 次，已登录云账号的再多 2 次；之后的升级不会再弹。

## [1.3.6] — 2026-10-06

### Added
- **Golden Buddha desk pet** (Pro): a seated golden statue, twice the size of the loong (Options → Effects → Desk Pet). It stays still. Idle and typing keep a soft light around the head; Enter (and connecting) widens that light over the whole figure, then lets it settle. The light is transparent and fades out at the edge, so the window shows through. No projectile. Position saved as `desk_pet_buddha_x` / `desk_pet_buddha_y`.
- **Taishang Laojun desk pet** (Pro): a seated sage (Options → Effects → Desk Pet). Idle and typing light the eight trigrams one by one on the wheel behind him. Enter and connecting light all eight. Purple qi drifts around the figure; the yin-yang turns slowly. The figure itself stays still. Position saved as `desk_pet_laojun_x` / `desk_pet_laojun_y`.
- **14-day Pro trial**: every install can use the local Pro features — the larger scrollback cap and the Monkey, Golden Buddha, and Laojun pets — for 14 days from first launch. Afterwards they need VsTerm Pro; a Pro pet that is in use switches back to the dog. The desk pet menu and Preferences → Account show the days left. Cloud sync still requires a Pro entitlement.

### Fixed
- **Laojun CPU use**: the pet kept the whole window repainting at 30 FPS. It now repaints at 10 FPS, and 5 FPS while the window is in the background. With software rendering, idle CPU drops from about 58% of a core to 37% (18% in the background).
- **Locked Pro pets**: hovering a locked pet in the menu now shows why it is locked. The hint never appeared before.

### 中文
- **新增**：桌宠「金佛」（Pro）——金色坐像，大小是青金龙的两倍（选项 → 特效 → 桌面宠物）。人像始终不动。闲置和打字只有头部一层渐渐淡出的金光；回车和连上主机时金光扩大到全身，边缘淡出，再慢慢收回。金光是透明的，后面的窗口能透出来。不发射光球。位置记在 `desk_pet_buddha_x` / `desk_pet_buddha_y`。
- **新增**：桌宠「太上老君」（Pro，选项 → 特效 → 桌面宠物）。闲置和打字时，背后八卦逐个点亮；回车和连上主机时八个全亮。周身紫气缓慢飘动，太极缓慢旋转，人物保持静坐。位置记在 `desk_pet_laojun_x` / `desk_pet_laojun_y`。
- **新增**：14 天 Pro 免费试用。从首次启动起 14 天内，可以免费使用本地 Pro 功能：更大的回滚行数上限，以及猴子、金佛、太上老君桌宠。到期后需要开通 VsTerm Pro；正在使用的 Pro 桌宠会切回小狗。桌宠菜单和「偏好设置 → 账号」会显示剩余天数。云同步仍需要 Pro 权益。
- **修复**：太上老君会让整个窗口按 30 FPS 持续重绘，现在改为 10 FPS，窗口在后台时 5 FPS。软件渲染下空闲 CPU 从约 58% 单核降到 37%（后台 18%）。
- **修复**：鼠标悬停在锁定的 Pro 桌宠上时会显示原因，以前这个提示从不出现。

## [1.3.5] — 2026-09-29

### Added
- **Drag to reorder the server list**: drag a server or a folder. The top or bottom edge of a row inserts before or after that item; the middle of a folder moves the item into it. While dragging something out of a folder, a bar at the top of the list drops it at the end of the root. The order is saved in `tree.yaml`. Folders still nest at most two levels, and a folder that already has a subfolder cannot be nested further. Dragging is off while the list is filtered, because hidden rows would make the drop index wrong.

### Fixed
- **Command suggestions while recalling history**: pressing Up or Down to recall a previous command no longer opens the suggestion popup, so the next arrows stay with the shell. The popup still opens when you type, paste, or delete on the prompt. Once it is open, Up and Down still move the selection.

### 中文
- **新增**：服务器列表可以拖动排序。拖到某一行的上沿或下沿，插到该项前面或后面；拖到文件夹行中间，移进这个文件夹。从文件夹里往外拖时，列表上方会出现「放到根目录」，松手后放到根目录末尾。顺序写入 `tree.yaml`。文件夹仍然最多嵌套两层，已经有子文件夹的不能再套进别的文件夹。搜索过滤开着的时候不能拖，因为这时看到的不是完整列表。
- **修复**：按上下键翻历史命令时不再弹出命令提示，后续的上下键继续发给终端。打字、粘贴、退格或删除时仍会出现提示；提示已经打开时，上下键仍用来选择建议。

## [1.3.4] — 2026-09-22

### Fixed
- **Chinese paste and input**: a single-line paste is sent as the characters themselves, so the shell does not paint the line in standout. On Windows the local PowerShell keeps its colors when the console is switched to UTF-8. A remote shell that has no UTF-8 locale (including BusyBox on Merlin / Dropbear) is started in `C.UTF-8`, and bash is told to echo multibyte input, so pasted and typed Chinese show up on the command line. `echo` of the same text already worked.
- **Copying Chinese out of the terminal**: a CJK glyph occupies two cells, and the trailing cell was copied as a space. Pasting that back produced `你 好`. On Windows PowerShell, `echo` splits on those spaces, so each character printed on its own line. The spacer cell is skipped; a space you actually typed is kept.
- **SFTP drag onto the terminal**: a path is quoted only when it is empty or contains whitespace or shell metacharacters. `/etc/hosts` and `/home/文档` are inserted as-is. A name with spaces or quotes is still one single-quoted argument.

### 中文
- **修复**：粘贴和输入中文。单行粘贴直接送字符，不再触发整行反白。本机 PowerShell 切到 UTF-8 时保留原来的颜色。远端如果没有 UTF-8 locale（包括梅林上的 BusyBox / Dropbear），启动时补上 `C.UTF-8`，bash 也会回显多字节输入，所以粘贴和输入的中文会出现在命令行上。`echo` 打出的中文本来就是对的。
- **修复**：从终端复制中文。汉字占两格，后半格以前被当成空格复制出去，贴回去就变成「你 好」。Windows 上 PowerShell 的 `echo` 会按这些空格拆参数，于是一个字一行。现在跳过占位格；你自己输入的空格仍会保留。
- **修复**：拖到终端的路径只在为空、含空格或含 shell 元字符时加引号。`/etc/hosts`、`/home/文档` 原样插入；带空格或引号的名字仍然是一个单引号参数。

## [1.3.3] — 2026-09-19

### Added
- **SFTP Copy Path**: right-click a remote file or folder (list or directory tree) to copy its full path to the clipboard. Multi-select copies every selected path, one per line, so they can be pasted into a terminal or another editor.
- **SFTP drag onto the terminal**: drag a remote row onto the terminal pane to insert the quoted path at the shell's current input. Several selected items become space-separated quoted paths; names with spaces or quotes stay one argument.
- **Scrollback memory budget**: terminal history is now bounded by memory rather than by line count alone. A history row costs roughly 24 bytes per column, so the old 100 000-line ceiling meant ~205 MB at 80 columns but ~490 MB at 200 — per tab, with nothing watching. Preferences → General sets a per-terminal budget (32 / 64 / 128 / 256 / 512 MiB, default 64, or unlimited for the previous behaviour) and shows the live total; the terminal's Clear scrollback submenu shows the active tab's figure. The budget is re-applied when a window is widened, because the same line count costs more once rows get wider.

### Changed
- **Startup memory**: ~63 MiB less resident at idle on a machine with a system CJK UI face (282 MB → 218 MB measured on Linux; Windows and macOS always take that path). Three causes: every owned font face was resident twice because `FontData::from_owned` makes egui clone the bytes again for its rasterizer, the app loaded three CJK faces regardless of UI language, and `Vault::try_open` re-ran a 19 MiB Argon2id pass on startup and two or three more times per connect just to test whether a master password is set. Faces are now handed over as `'static`, only the primary CJK face is loaded, and the password probe is cached against the current DEK wrap.
- **Connect latency**: opening a session no longer stalls on two or three Argon2id derivations (tens of milliseconds each).
- **Monitor sampling**: the process table no longer resolves every process's executable path and disk usage once a second — it only shows pid, name, CPU and memory.

### Fixed
- **Imported Windows key paths** are shown by file name again on macOS and Linux. The credential picker asked `Path::file_name` for the last component, which only knows the host's separator, so a path like `C:\Users\vesaa\.ssh\id_ed25519.pem` was treated as one long name and truncated from the drive letter (`C:\Users\vesaa\.s….pem`). Xshell, FinalShell and SecureCRT imports keep `UserKey` paths verbatim, so this showed up on any cross-platform import.

### Removed
- **Korean UI language**. Existing `locale: ko` configs fall back to English. Japanese is unaffected: YaHei / PingFang SC / Noto Sans SC all cover kana, so the separate Japanese face only changed kanji from SC to JP shapes and was not worth 10–20 MB.

### 中文
- **新增**：SFTP 右键「复制路径」——文件列表或左侧目录树右键即可把完整远端路径写入剪贴板；多选时一行一条，方便贴到终端或其它编辑器。
- **新增**：把 SFTP 文件/文件夹拖到终端区域，会在当前输入位置插入带引号的路径；多选则空格分隔，含空格或引号的名字仍是一个参数。
- **新增**：终端回滚缓冲区改为按内存限制，而不再只看行数。一行历史约占「24 字节 × 列数」，所以原来的 10 万行上限在 80 列下是约 205 MB，200 列下就是约 490 MB —— 而且是每个标签页各算一份，此前没有任何约束。「选项 → 偏好设置 → 常规」可以设置每个终端的上限（32 / 64 / 128 / 256 / 512 MiB，默认 64，也可选不限制以保持旧行为），旁边显示所有标签页的实时合计；终端右键的「清理滚动历史」子菜单里显示当前标签页的占用。窗口拉宽后会重新收敛，因为行变宽之后同样的行数会更贵。
- **变更**：启动常驻内存下降约 63 MiB（Linux 实测 282 MB → 218 MB；Windows 和 macOS 必然走同一条路径）。三个原因：每个系统字体面在内存里存了两份（`FontData::from_owned` 会让 egui 为光栅化再克隆一份）；不论界面语言都加载三个 CJK 字体面；`Vault::try_open` 仅仅为了判断有没有主密码，就在启动时跑一次 19 MiB 的 Argon2id，每次连接再跑两三次。现在字体面按 `'static` 交给 egui、只加载主 CJK 面、密码探测按当前密钥封装缓存。
- **变更**：连接会话不再卡在两三次 Argon2id 推导上。
- **变更**：监视器进程表不再每秒为每个进程解析可执行文件路径和磁盘用量。
- **修复**：macOS / Linux 上导入的 Windows 私钥路径重新按文件名显示。凭证选择器用 `Path::file_name` 取末段，而它只认宿主平台的分隔符，于是 `C:\Users\vesaa\.ssh\id_ed25519.pem` 被当成一个超长文件名，从盘符开始截断成 `C:\Users\vesaa\.s….pem`。Xshell / FinalShell / SecureCRT 导入会原样保留 `UserKey` 路径，所以任何跨平台导入都会碰到。
- **移除**：韩语界面。已有配置里的 `locale: ko` 会回落到英文。日语不受影响：雅黑 / 苹方 / Noto Sans SC 本身都含假名，单独的日文面只改变汉字字形（SC 形 → JP 形），不值 10–20 MB。

## [1.3.2] — 2026-09-17

### Added
- **Inline command suggestions**: after a couple of characters, matching local history (and saved quick commands) appears in a popup above the caret. Arrow keys / click / Enter fill the remainder without running it. History is encrypted on disk (`cmdhist.enc`), never cloud-synced, and commands that look like they contain secrets are dropped.

### Fixed
- **Command history truncation**: wrapped long commands are joined when recording (toolbar History and the new suggestions), so the stored text is the full line rather than the first row. Overlong rows in both panels ellipsize instead of overflowing.
- **CJK vs ASCII cell alignment**: each terminal glyph is optically centered in the cell using its ink (`Galley::mesh_bounds`). `LEFT_CENTER` only centers the shared layout box, so Chinese still hugged the top while digits/Latin sat mid-cell ([#22](https://github.com/vesaaa/vsterm/issues/22)). CJK is the OS UI face (PingFang SC on macOS, YaHei on Windows, Noto on Linux) — macOS does not use YaHei.
- **Suggestion “quick” tag**: the CJK/Latin label is laid out as its own centered galley instead of sharing a LayoutJob with the monospace command, so it stays on the same baseline (Windows 125% DPI).

### 中文
- **新增**：终端输入联想。打几个字符后，本地历史（以及快捷命令）在光标上方弹出；方向键 / 点击 / Enter 填充剩余字符，不执行。历史加密保存在本机，不同步云端；像密码、令牌的命令不会入库。
- **修复**：折行的长命令现在会拼成完整一条再记入右上角历史和新的联想；超长文本在面板里省略，不再撑破宽度。
- **修复**：同一行里中文靠顶、英文/数字居中（[#22](https://github.com/vesaaa/vsterm/issues/22)）。按每个字形的墨水盒垂直居中，而不是居中整行 layout 盒。macOS 中文是苹方，不是雅黑。
- **修复**：联想列表里「快捷」不再和命令错行（Win10 125% DPI 下中英混排基线不一致）。

## [1.3.1] — 2026-09-16

### Added
- **SSH client proxy**: reusable SOCKS5 / HTTP CONNECT catalog (same pattern as credentials). Add/Edit Server has one proxy row (default None); picking a proxy opens the list. Manage multiple named proxies via Options → Proxy Library. Direct TCP is unchanged; VsTerm does not use the OS / `HTTP_PROXY` system proxy for SSH. SOCKS5 sends the hostname to the proxy (remote DNS). OpenSSH `ProxyCommand` (`nc`/`ncat`/`connect`/`corkscrew`), Xshell, and FinalShell proxy fields are imported into the catalog.

### Changed
- **Proxy Library**: each saved proxy can be edited and tested (SOCKS5 method negotiation, or HTTP CONNECT). Leave the password blank when editing to keep the saved secret. List chips sit next to row-height icon actions (test / edit / delete); Credential Vault delete uses the same icon button, plus an eye to peek at a password. **Show passwords** next to Add Credential is remembered in `config.yaml` until unchecked, and reveals every secret in the vault list and in the Add Server credential picker.

### 中文
- **变更**：添加/编辑服务器只保留一行选择代理（默认「无」）；点击打开代理列表，双击选用。多条代理在「选项 → 代理库」里配置，交互与凭证库一致。代理库支持编辑与测试（SOCKS5 握手 / HTTP CONNECT）；编辑时密码留空则保留已保存密码。代理库列表加宽贴齐操作按钮，测试/编辑/删除改为与行等高的图标；凭证库列表铺满宽度，删除为图标，左侧增加查看密码（眼睛）；「添加凭证」旁可勾选「不隐藏密码」（写入配置，取消勾选前一直生效），凭证库与添加主机的凭证列表都会明文显示。
- **新增**：会话级 SOCKS5 / HTTP CONNECT 代理（与 Termius 的 Proxy 同类）。直连行为不变；SSH **不走** 系统代理或 `HTTP_PROXY`。SOCKS5 把主机名交给代理做远程 DNS。可从 OpenSSH `ProxyCommand`、Xshell、FinalShell 导入到代理库。

## [1.3.0] — 2026-09-15

### Changed
- **Release CI**: compile cache is sccache on the dedicated `R2_SCCACHE_*` bucket (`ci/sccache/`), not the CDN download bucket. v1.2.14 did not run sccache; this release is the first tag that should write cache objects. The first build is still a cold compile (`cache writes`); later tags with the same rustc/deps should hit.

### 中文
- **变更**：发版编译缓存改走独立的 `R2_SCCACHE_*` 桶（前缀 `ci/sccache/`）。v1.2.14 当时还没合进这段 workflow；这一发会第一次写入缓存（冷编译），下一发才可能命中。

## [1.2.14] — 2026-09-15

### Added
- **Software sources (toolbox)**: switch Debian/Ubuntu apt and RHEL-family yum/dnf mirrors (official, Aliyun, Tsinghua/USTC EDU, 163, Tencent, Huawei), plus optional Docker CE and Nginx *package* repos. Each apply renames originals on the host to `<name>.bak.<UTC>` (existing backups are kept) and can restore that file. OpenWrt/Merlin/Alpine/Windows are detected and left untouched.

### Changed
- **Software sources**: hide the empty VsTerm-backup hint; preview and current source files use a smaller muted monospaced panel; privilege follows a terminal `sudo -i` / `su` the same way the file panel does.
- **About**: lists the official site [www.vsterm.com](https://www.vsterm.com) alongside GitHub.
- **Account entitlements**: last successful `GET /v1/promo/channels` is cached on disk so the list paints immediately; a skeleton placeholder shows while the first fetch is in flight, then the live list replaces it.
- **Buy Pro**: Preferences → Account purchase action (and the unsigned fallback row) opens [https://www.vsterm.com/vsbuy](https://www.vsterm.com/vsbuy).

### Fixed
- **SFTP vs `sudo -i`**: typing `sudo -i` no longer pops the file-panel sudo password dialog (that stole the password from the PTY). The password goes to the terminal; SFTP stays the login user until you click Elevate (or when `sudo -n` is already enough).
- **Software sources after `sudo -i`**: Apply no longer wraps a new exec with `sudo -n` (which fails with `sudo: a password is required` because tty_tickets do not share the PTY timestamp). The script runs in the already-elevated terminal instead.

### 中文
- **变更**：软件源无备份时不再显示空说明；预览/当前源文件改为浅色小号等宽文本框；终端里 `sudo -i` 后与文件面板一样按提升后的身份探测与写入。
- **新增**：工具箱「软件源」——切换 Debian/Ubuntu apt 与 CentOS 7 / Rocky / Alma / Stream 的 yum/dnf 镜像（官方、阿里云、清华/中科大 EDU、163、腾讯、华为），可选 Docker CE / Nginx **软件包**仓库。每次应用在服务器把原文件改名为 `原名.bak.UTC时间戳`（不覆盖已有备份），可按备份恢复。OpenWrt / 梅林 / Alpine / Windows 只提示不支持，不改写。
- **关于**：补充官网地址 [www.vsterm.com](https://www.vsterm.com)。
- **开通权益**：上次成功拉取的渠道列表会缓存在本地，打开账号页先渲染缓存；没有缓存时用加载占位，服务端返回后再替换。
- **购买 Pro**：偏好配置 → 账号 可直接打开购买页，也可访问 [https://www.vsterm.com/vsbuy](https://www.vsterm.com/vsbuy)。
- **修复**：普通用户在终端输入 `sudo -i` 时，SFTP 提权窗口不再抢走密码；密码先给终端。SFTP 保持登录用户，需要时再点文件面板的提权（免密 sudo 仍会自动跟随）。
- **修复**：终端 `sudo -i` 成 root 后，软件源界面虽显示 root，点保存不再报 `sudo: a password is required`。写入改在已经提权的终端里执行（新的 SSH exec 仍是登录用户，且 tty_tickets 不能把 PTY 的 sudo 票据带到 exec）。

## [1.2.13] — 2026-09-09

### Fixed
- **SSH to Dropbear / older Linux**: enabling Shell integration no longer fails the whole login. The OSC bootstrap is ~10 KB `exec`; Dropbear’s command cap is ~9000 bytes and a longer string often **disconnects the session** (so retrying on another channel cannot recover). VsTerm now skips that exec on Dropbear / unknown banners and opens a normal login shell. If integration still fails after a fallback, the error is **Shell integration failed** (uncheck OSC path sync) — not a generic host/port/network failure. Password auth asks the server which methods it offers. Handshake also offers NIST ECDH, `diffie-hellman-group14-sha1`, `hmac-sha1`, and AES-CBC after the modern algorithms.
- **Account sign-in**: the verification-code field and Sign in button no longer sit on top of each other (the preferences shell zeros item spacing). They share one row with an 8px gap.

### Changed
- **Preferences window**: the dialog stays a fixed size but can be dragged by the title bar (it is no longer re-anchored to the center every frame).
- **Preferences action spacing**: Browse/Clear, Refresh/Sign out, Upload/Download, and the three device cards now use a 2px gap (same as the session-tree search/collapse icons).
- **Device cards**: left OS badge (Windows / macOS / Linux), right column with hostname on top and Revoke / “This device” aligned to the bottom-right.
- **Cloud upload guard**: uploading with fewer than 5 local sessions asks for confirmation (and to verify the local config is current) so a small local tree cannot silently overwrite cloud config.

### 中文
- **修复**：部分 Linux（尤其是梅林 / OpenWrt 等 Dropbear）勾选 OSC 路径同步后会提示「连接失败」，其它 SSH 软件却能连。集成脚本约 10KB，超过 Dropbear 命令上限时会直接掐掉整条会话。现在对 Dropbear 会改走普通登录 shell，勾选 OSC 也能进终端。若集成仍然失败，会单独提示「Shell 集成失败」（取消勾选 OSC），不再误导成主机/端口/网络不通。密码登录会先询问服务器支持的认证方式；算法协商补上 NIST ECDH、`diffie-hellman-group14-sha1`、`hmac-sha1`、AES-CBC。
- **修复**：账号页发送验证码后，验证码输入框与登录按钮不再重叠（偏好设置外壳把控件间距设为 0）；两者同一行并留 8px。
- **变更**：偏好设置窗口尺寸仍固定，可用标题栏拖动（不再每帧钉在屏幕正中）。
- **变更**：偏好设置里成对按钮（浏览/清除、刷新/退出登录、上传/下载）以及「我的设备」三张卡片间距统一为 2px。
- **变更**：设备卡片改为左 OS 图标、右主机名；「撤销」与「当前设备」都放在右下角。
- **变更**：本地服务器少于 5 台时，上传到云端会先确认（并提醒确认本地配置是最新的），避免误覆盖云端配置。

## [1.2.12] — 2026-09-08

### Changed
- **Native preferences window**: Options → Preferences is now a settings center with a square icon sidebar (General / Master password / Account / Cloud sync), vertical cards, and a fixed dialog size. Title bar and shadow match the main window.
- **External editor command line**: Preferences → General accepts a typed or pasted editor path, including extra flags (Notepad++ `-multiInst`, VS Code `--wait`, `%f` / `{file}` placeholders). A missing or crashing editor only shows an error and does not block the rest of VsTerm. Browse/Clear on that row use a 10px gap.

### 中文
- **变更**：偏好设置改为原生设置中心：左侧图标侧栏（常规 / 主密码 / 账号 / 云端同步）、纵向卡片、固定窗口尺寸；标题栏和阴影与主窗口一致。
- **变更**：外部编辑器支持手动输入/粘贴，并可带启动参数（如 Notepad++ `-multiInst`、VS Code `--wait`，以及 `%f` / `{file}`）。程序不存在或启动失败只提示错误，不影响其它功能。浏览/清除间距为 10px。

## [1.2.11] — 2026-09-07

### Added
- **Jade loong desk pet**: original teal-gold coiled Chinese loong (Options → Effects → Desk Pet). Idle pose matches the concept (tail coiled on the left, one paw on the ground pearl); typing uses a tiny keyboard; Enter opens the mouth and the pearl flies from the snout (same projectile as the qi fighter). Free tier; position saved as `desk_pet_dragon_x` / `desk_pet_dragon_y`.
- **SFTP external editor**: “Open with editor” downloads the remote file as raw bytes, opens a user-chosen editor, and uploads the saved bytes on disk change. No encoding or newline conversion. The editor path lives in machine-local `vsterm.conf` (next to the binary when writable, otherwise the platform config dir) and is **not** cloud-synced. First use prompts for an editor; **Options → Preferences → General** can set or clear it.

### Fixed
- **Host import encoding**: MobaXterm (and other ANSI session files) from Chinese Windows no longer show mojibake such as `Éú²ú` for `生产`. Files without a BOM are decoded as UTF-8 when valid, otherwise GB18030 / Big5 when the text looks like CJK, and Windows-1252 as a last resort. WindTerm, Xshell, FinalShell, Tabby, OpenSSH, and SecureCRT use the same decoder.

### 中文
- **新增**：桌宠「青金龙」——原创青绿金角螭龙（选项 → 特效 → 桌面宠物）。闲置盘尾在左、一爪踩地珠；打字用小键盘；回车张嘴，珠从嘴前飞出（与气功武者同一弹道）。免费；位置记在 `desk_pet_dragon_x` / `desk_pet_dragon_y`。
- **新增**：SFTP「使用编辑器打开」按字节下载到临时文件，用用户指定的外部编辑器打开，保存后按字节回写远端（不做编码/换行转换）。编辑器路径写在本机 `vsterm.conf`（能写则放程序目录，否则平台配置目录），**不同步云端**。首次使用会选择编辑器；也可在 **选项 → 偏好设置 → 常规** 里设置或清除。
- **修复**：从中文 Windows 的 MobaXterm（以及其他 ANSI 会话文件）导入时，`生产` 不再显示成 `Éú²ú`。无 BOM 文件优先 UTF-8，否则按 GB18030 / Big5 识别中文，最后才回退 Windows-1252。WindTerm / Xshell / FinalShell / Tabby / OpenSSH / SecureCRT 使用同一解码。

## [1.2.10] — 2026-09-06

### Fixed
- **Remote macOS shell integration**: connecting with Shell integration enabled no longer fails with `/bin/sh: syntax error near unexpected token ;;`. macOS `/bin/sh` is bash 3.2, which treats `)` in `case pat)` inside `$(...)` as the end of the substitution. Last-login scanning now keeps `case` in a helper function, and uses BSD `last -5` (with GNU `last -n 5` as fallback).
- **CJK font RSS**: system `.ttc` collections load a single extracted face instead of the whole file (macOS PingFang / Apple SD Gothic / Hiragino, Windows YaHei, Linux Noto CJK). On this machine PingFang dropped from ~74 MB to ~13 MB in-process.

### 中文
- **修复**：开启 Shell 集成连接远端 macOS 时，`/bin/sh`（bash 3.2）报 `syntax error near unexpected token ;;`。Last login 探测把 `case` 挪出 `$(...)`，并兼容 BSD `last -5`。
- **修复**：系统 TTC 字体只抽取当前字重进内存，不再整包加载 PingFang / 雅黑 / Noto CJK 合集。

## [1.2.9] — 2026-09-05

### Changed
- **Account sign-in**: the unsigned-in Account tab now explains that Pro subscription, cloud sync, and device management require sign-in, and that the email only receives a login verification code (no password). SSH connections do not need a cloud account.

### Fixed
- **macOS language menu**: Korean (`한국어`) no longer renders as tofu. PingFang SC has no Hangul coverage; the UI font stack now falls back to Apple SD Gothic Neo (and Hiragino for Japanese), matching the existing Windows Malgun / Yu Gothic fallbacks. Linux loads Noto Sans CJK KR when present.
- **OS keyring prompts**: cloud session fields (token, expiry, account id/email, device id, fingerprint, auth-block) are stored in a single `cloud-session` item instead of ~7 separate entries. Existing split items are migrated on first read. Vault “remember unlock” and the device Ed25519 key stay separate; in-process caches skip repeat `get_password` calls (macOS / Windows / Linux). Signing in writes the session blob once.
- **macOS Network Connections**: GUI-launched apps often have no `/usr/sbin` on `PATH`, so `netstat` was never found. BSD `netstat -an` also uses `tcp4`/`udp6` and `host.port` instead of Linux `tcp` / `host:port`, so every chart stayed at 0. The collector now locates `netstat` the same way path-trace finds `traceroute`, and the parser accepts the BSD rows.
- **macOS Path Trace**: Auto/IPv4 against a literal such as the default `8.8.4.4` passed GNU-only `-4` to Apple traceroute (`invalid option -- 4`), so the panel showed “未能解析到路径跳点”. The probe now drops address-family flags the local tool rejects, merges traceroute stderr, and still parses classic BSD `host (ip)` / `host [ip]` hop lines.
- **Path Trace mtr**: report mode no longer passes `-z`. That ASN column replaces the `|---` host marker, so every hop rendered as `*` and extra IPv4 continuation lines became fake hop numbers. The parser also accepts `AS???` / `ASnnnn` rows if `-z` is present.
- **Private-key picker** no longer filters to `.key` / `.pem` / `.ppk`. OpenSSH keys without a suffix (`id_rsa`, `id_ed25519`, self-built names) are selectable. A **Paste** button next to Browse fills a file path from the clipboard.

### 中文
- **改进**：未登录账号页补充说明：登录后才可订阅 Pro、使用云端同步并管理设备；邮箱仅用于接收登录验证码，无需设置密码。SSH 连主机不依赖云账号。
- **修复**：macOS 多语言菜单中韩文不再乱码；在 PingFang SC 后追加 Apple SD Gothic Neo（日文追加 Hiragino）。
- **修复**：云会话相关钥匙串合并为一条 `cloud-session`，启动时不再连续弹出约 7 次授权。Vault 记住解锁与设备密钥仍分条保存；进程内缓存避免重复读取。登录只写一次会话条目。Windows 凭据管理器 / Linux Secret Service 同样减少读写。
- **修复**：macOS「网络连接」TCP/UDP 图表一直为 0。从 Finder 启动时 `PATH` 常不含 `/usr/sbin`（`netstat` 在那里）；即便采到输出，解析器也不认 BSD 的 `tcp4` / `192.168.1.1.443`。现已按绝对路径找 `netstat` 并解析该格式。
- **修复**：macOS「路由追踪」对 IPv4 字面量（默认 `8.8.4.4`）会把 GNU 的 `-4` 传给 BSD `traceroute`，报 `invalid option -- 4`，界面只显示「未能解析到路径跳点」。现会探测工具是否接受该参数，并合并 stderr、解析 BSD 跳点行。
- **修复**：路径追踪的 mtr 报告不再带 `-z`（ASN 列会吃掉 `|---`，跳点地址全变成 `*`）。解析器同时兼容 `AS???` 行。
- **修复**：私钥「浏览」不再只显示 `.key` / `.pem` / `.ppk`；无后缀密钥（`id_rsa`、自建文件名）可选。路径旁新增「粘贴」，用于粘贴文件路径。

## [1.2.8] — 2026-09-03

### Fixed
- **macOS release packaging** now ad-hoc signs the assembled `.app`, verifies the signature in CI, and stages the bundle with `ditto` so Apple Silicon no longer hits the fatal “damaged” path for unsigned builds.
- **Remote Bash shell integration** now trims stray leading/trailing semicolons and whitespace before installing the `PROMPT_COMMAND` hook, avoiding the repeated `syntax error near unexpected token ';'` prompt spam seen on hosts with profiles like `history -a;`.
- **Shell hook installation hardening**: Bash array `PROMPT_COMMAND` is deduped element-wise and prepended so `$?` stays accurate; Zsh fallback hook arrays now dedupe too.

### 中文
- **修复**：macOS 发版打包现在会对组装后的 `.app` 做 ad-hoc 签名，在 CI 中校验签名，并用 `ditto` 进入 DMG 暂存目录，避免 Apple Silicon 把未公证构建直接判成“已损坏”。
- **修复**：远端 Bash 的 shell 集成在安装 `PROMPT_COMMAND` hook 前会清理首尾多余分号和空白，避免 `history -a;` 这类配置导致每次提示符前重复报 `syntax error near unexpected token ';'`。
- **修复**：Shell hook 安装加固：Bash 数组形式的 `PROMPT_COMMAND` 按元素去重并前置，确保 `$?` 保持正确；Zsh fallback 的 hook 数组也会去重。

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
