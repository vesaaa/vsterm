# VsTerm

[English](README.md) | **简体中文**

<p align="center">
  <img src="assets/branding/logo.png" alt="VsTerm" width="160" height="160">
</p>

<p align="center">
  <strong>跨平台 SSH 终端管理工具</strong>（Rust 原生）
</p>

<p align="center">
  集合 <strong>WindTerm</strong> / <strong>Termius</strong> / <strong>FinalShell</strong> 三大主流工具的优点：<br>
  专业终端体验 · 现代会话与凭据管理 · 文件传输与运维一体化
</p>

- GUI：`egui` + `eframe`（`wgpu`：Windows DX12 / macOS Metal / Linux Vulkan）
- 终端仿真：`alacritty_terminal`
- SSH：内置 `russh`（终端、远程命令与 SFTP 复用同一认证会话）
- 配置：YAML；凭据：系统 keyring + 加密 vault

## 特色功能

### 1.1.13 版本现在最突出的点

- **终端、文件、传输在一个工作流里**：Shell 标签、底部 SFTP 文件区、sudo 提权文件传输、ZMODEM 收发都围绕同一条 SSH 会话组织，不需要来回切工具。
- **运维面板不是附属品，而是主功能**：路由信息、网络连接、路径追踪、IP 质量、系统信息、实时指标都内置在主界面。
- **传输过程看得见**：**SFTP** 和 **ZMODEM（`rz` / `sz`）** 都有明确的进度 / 队列反馈，而不是只弹一个阻塞对话框或静默后台执行。
- **线路与网络定位能力明显强于普通终端**：不仅能连，还能看策略路由图、逐跳地理归属 / ASN、IP 欺诈分 / 大厂身份 / 黑名单结果。
- **原生桌面应用**：不是 Electron；Rust + `wgpu`，在无硬件 GPU 的 VM / RDP 环境下也能自动降级保证可用。

### 截图预览

#### 总体终端工作区

<img src="assets/screenshots/overview_terminal.webp" alt="VsTerm 终端总览，包含会话树、终端区与底部面板" />

#### 路由图 / 策略拓扑

<img src="assets/screenshots/routes_diagram.webp" alt="VsTerm 路由图与策略拓扑" />

#### IP 质量检测

<img src="assets/screenshots/ip_quality.webp" alt="VsTerm IP 质量检测面板，包含风险、组织、归属地与黑名单结果" />

#### 带归属地 / ASN 的路径追踪

<img src="assets/screenshots/path_trace_geo.webp" alt="VsTerm 路径追踪，显示 RTT、丢包、组织、ASN、归属地与坐标" />

### 主流 SSH / 终端工具能力对比

说明：`✅` = 内建且是第一层工作流，`◐` = 支持但偏附属 / 伴生工具 / 插件式入口，`✗` = 不是该产品常规主打的内建能力。

| 能力 | VsTerm | WindTerm | Termius | FinalShell | MobaXterm | SecureCRT | Xshell | Tabby |
|------|--------|----------|---------|------------|-----------|-----------|--------|-------|
| 开发语言 | Rust | C/C++ | Electron | Java | C++ | C++ | C++ | Electron |
| 终端历史最大行数 | 10万 / 50万（Pro） | unlimited | - | - | 360,000 | 128,000 | ~2.1B | 25,000 |
| 终端输出代码折叠 / outline | ✅ | ✅ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |
| 内建 SFTP 文件面板 / 远端文件管理 | ✅ | ✅ | ✅ | ✅ | ✅ | ◐ | ◐ | ◐ |
| SFTP 传输进度 / 队列可见 | ✅ | ◐ | ◐ | ✅ | ◐ | ◐ | ◐ | ◐ |
| 内建 ZMODEM（`rz` / `sz`） | ✅ | ✅ | ✗ | ✅ | ◐ | ✅ | ✅ | ✅ |
| ZMODEM 进度在界面中可见 | ✅ | ✅ | ✗ | ◐ | ◐ | ◐ | ◐ | ◐ |
| 终端 ↔ 文件区路径双向同步 | ✅ | ✗ | ◐ | ◐ | ✗ | ✗ | ✗ | ✗ |
| 文件区可跟随 `sudo -i` / `su` 提权身份 | ✅ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |
| 图形路由 / 策略路由拓扑 | ✅ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |
| 带归属地 / ASN 的路径追踪 | ✅ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |
| 内建 IP 质量 / IP 信誉检测 | ✅ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |
| CPU / 内存 / 存储图形监控 | ✅ | ◐ | ✗ | ✅ | ✗ | ✗ | ✗ | ✗ |
| 连接数 / socket 监控面板 | ✅ | ✗ | ✗ | ◐ | ✗ | ✗ | ✗ | ✗ |
| 连接特效 / 动效打磨 | ✅ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |
| 桌面宠物 | ✅ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |

补充说明：

- 这里是按 **用户可直接看到的内建能力** 来比，不把“手动敲命令能做到”算成产品特性。
- 一些工具的文件传输体验依赖伴生程序、独立标签页或插件，所以标成 `◐` 而不是 `✅`。
- 历史行数优先采用公开文档中的默认值 / 上限；查不到就标 `-`。VsTerm 会动态扩容：**标准版 100,000** 行，**Pro 500,000** 行。

### 标准版 vs Pro

VsTerm 本地 SSH 功能可完全离线使用。**Personal Cloud**（账号登录、权益与加密同步）为可选项。**Pro** 需通过绑定校验的云端权益解锁（例如 GitHub Star 活动或兑换码）。

| 能力 | 标准版 | Pro |
|------|--------|-----|
| 本地 SSH 会话、SFTP、ZMODEM、运维面板 | ✅ | ✅ |
| 本机加密凭据 vault | ✅ | ✅ |
| 终端历史最大行数 | 100,000 行 | 500,000 行 |
| 桌面宠物 — 小狗（整窗自由拖放） | ✅ | ✅ |
| 桌面宠物 — 猴子（整窗自由拖放） | ✗ | ✅ |
| Personal Cloud 账号 / 设备管理 | ✅ | ✅ |
| 云端同步（会话、命令、布局、偏好、凭据） | ✗ | ✅ |
| 后续增强功能 | 不支持 | 持续支持 |

#### 云端同步与数据安全

同步内容均为**客户端加密后再上传**。服务端只保存密文，无法读取你的会话、命令、布局、偏好或 vault 内容。

| 层级 | 算法 / 模型 |
|------|-------------|
| 设备身份（本地密钥对） | **Ed25519** — 私钥仅保存在系统钥匙串，从不上传 |
| 同步内容加密 | **AES-256-GCM**，密钥由**主密码**派生（Argon2id → VSC2 密文包） |
| 同步中的凭据 vault | 本机已加密；同步上传的是密封后的 `vault.enc`，不是明文密码 |

没有你的主密码（及由其派生的同步密钥），云端密文无法解密——包括 VsTerm 运营方也无法读取。

### 宠物与连接特效——运维不再枯燥

- **桌面宠物**：小狗（标准版）/ 猴子（Pro）；可在整窗任意拖放；对打字、回车、连上主机有姿态反应
- **连接特效**：拖影吸入 / 破碎重组；连上后标签 accent 扫光
- 无硬件 GPU（如部分 RDP / WARP）时自动降级帧率，保证稳定可用

## 首次运行

从 [Releases](https://github.com/vesaaa/vsterm/releases) 下载对应平台包并解压后运行。首次启动会在用户目录下创建 `~/.vsterm/`（含演示会话树）。

若要从其他终端迁主机列表：菜单 **文件 → 从其他导入**，再选 **WindTerm**（`.wind` / `profiles/`）、**Xshell**（带 `.xsh` 的 `Sessions`）、**FinalShell**（数据目录或 `conn/`）、**MobaXterm**（`MobaXterm.ini` / `.mxtsessions`）、**Tabby**（`config.yaml`）、**OpenSSH**（`~/.ssh/config`）或 **SecureCRT**（带 `.ini` 的 `Sessions`）。只导入 SSH 主机与分组（最多两层文件夹）。密码因对方加密无法导入，首次连接时再输入。私钥**文件路径**会保留公钥登录；Xshell / FinalShell 密钥管理器里的名称无法解析。

当前发布包尚未经过微软 / Apple 商业签名公证，系统可能拦截——按下面步骤放行即可（仅需一次）。

### Windows（SmartScreen 拦截）

1. 解压后双击 `VsTerm.exe`
2. 若出现「Windows 已保护你的电脑」/ SmartScreen 提示，点 **更多信息**
3. 再点 **仍要运行**

也可：右键 `VsTerm.exe` → **属性** → 勾选 **解除锁定** → 确定后再打开。

### macOS（无法打开 / 已损坏类提示）

1. 下载对应芯片的 DMG：Apple Silicon 用 `vsterm-macos-arm64.dmg`，Intel 用 `vsterm-macos-x64.dmg`
2. 打开 DMG，将 **VsTerm** 拖到 **应用程序**（GeoIP 等运行时数据已打进 `.app`，无需再拷 `tools/`）
3. 若双击提示无法打开或来自未识别开发者：打开 **系统设置 → 隐私与安全性**，在下方找到被拦截的 VsTerm，点 **仍要打开**
4. 若仍被 Gatekeeper / quarantine 拦住，在终端执行一次清除隔离属性后再打开：

```bash
xattr -cr /Applications/VsTerm.app
```
### Linux

解压后赋予执行权限并运行：

```bash
chmod +x VsTerm
./VsTerm
```

## 字体许可

内嵌字体均为 SIL OFL 1.1，许可文件见 `assets/fonts/`。

## ⚠️ 授权声明 (License & Copyright)

本项目的源代码基于 **保留所有权利（All Rights Reserved）**，并参考 **CC BY-NC-ND 4.0** 的非商业、禁止演绎精神进行约束。完整条款见仓库根目录 [`LICENSE`](LICENSE)。

**著作权人：** vesaa（源代码、文档、设计之著作权及知识产权均归作者所有；未经授权不得对衍生版本主张独立知识产权或二次打包售卖）

| 用途 | 是否允许 |
|------|----------|
| 个人学习与研究 | ✅ 可免费查看源码、本地编译运行 |
| 修改 / fork / 二次打包发布 | ❌ 未经书面授权禁止 |
| 魔改后申请知识产权或 rebranding 售卖 | ❌ 未经书面授权禁止 |
| 集成到其他项目（含闭源） | ❌ 未经书面授权禁止 |
| 商业盈利 / 企业生产部署 / 商业交付 | ❌ 须事先购买商业授权 |

**商业授权联系：** [vesaazheng@gmail.com](mailto:vesaazheng@gmail.com)

> 说明：本项目依赖的第三方开源库及内嵌字体，仍分别受其各自许可证约束。
