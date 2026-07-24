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

### 终端 × SFTP 双向路径同步，支持提权

文件区与终端不再各走各的：

- **文件 → 终端**：在远端目录浏览时一键 `cd`，路径立刻同步到当前 Shell
- **终端 → 文件**：依赖 OSC 7 上报 cwd，终端切目录后文件区自动跟随
- **sudo / 提权 SFTP**：终端里 `sudo -i` / `su` 提权后，文件传输可切到同一身份访问受限路径；也可一键 Elevate，密码对话框安全录入
- 真 SFTP：列表、导航、单文件与目录上下传、进度与队列；支持拖放、新建/重命名/删除

### 集百家之长，走特色路线

| 借鉴自 | VsTerm 落地 |
|--------|-------------|
| **WindTerm** | 命令块折叠 gutter、时间戳与行号、语义高亮、大滚动历史 |
| **Termius** | 会话树与文件夹、跨端友好的 YAML 配置、加密 vault / 主密码 |
| **FinalShell** | 底部文件管理、主机侧运维工具箱、可视化性能 / 网络 / 存储监控 |

另有：快速连接（`user@host` / `ssh://…`）、多标签与重连、本地 Shell、ZMODEM（`rz`/`sz`）、系统监控与系统信息面板。

### 宠物与连接特效——运维不再枯燥

- **桌面宠物**：猴子（右边框）/ 小狗（底部边框），可拖动；对打字、回车、连上主机有姿态反应
- **连接特效**：拖影吸入 / 破碎重组；连上后标签 accent 扫光
- 无硬件 GPU（如部分 RDP / WARP）时自动降级帧率，保证稳定可用

### 增强的网络工具

主区内置运维级网络面板，本地与远端均可查看：

- **路由信息**：完整呈现 IPv4 / IPv6 路由表与 `ip rule` 策略规则，主机多出口、策略路由一览无余，排查走哪条线不再靠猜
- **网络连接**：TCP / UDP 连接数总览；本机与远端主机连接详情；路由器 NAT 转发连接明细与筛选，快速定位谁在连、谁在被转
- **路径追踪**：逐跳 RTT / 丢包，并标注线路 ASN、组织、归属地与经纬度，便于线路质量排查与出口优化

## 首次运行

从 [Releases](https://github.com/vesaaa/vsterm/releases) 下载对应平台包并解压后运行。首次启动会在用户目录下创建 `~/.vsterm/`（含演示会话树）。

当前发布包尚未经过微软 / Apple 商业签名公证，系统可能拦截——按下面步骤放行即可（仅需一次）。

### Windows（SmartScreen 拦截）

1. 解压后双击 `VsTerm.exe`
2. 若出现「Windows 已保护你的电脑」/ SmartScreen 提示，点 **更多信息**
3. 再点 **仍要运行**

也可：右键 `VsTerm.exe` → **属性** → 勾选 **解除锁定** → 确定后再打开。

### macOS（无法打开 / 已损坏类提示）

1. 解压对应芯片包：Apple Silicon 用 `vsterm-macos-arm64`，Intel 用 `vsterm-macos-x64`
2. 若双击提示无法打开或来自未识别开发者：打开 **系统设置 → 隐私与安全性**，在下方找到被拦截的 VsTerm，点 **仍要打开**
3. 若仍被 Gatekeeper / quarantine 拦住，在终端执行一次清除隔离属性后再打开：

```bash
xattr -cr /path/to/VsTerm
```

将 `/path/to/VsTerm` 换成实际解压路径。之后即可正常双击启动。

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
