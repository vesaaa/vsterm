# VsTerm

跨平台 SSH 终端管理工具（Rust 原生），对标 WindTerm / Termius。

- GUI：`egui` + `eframe`（`wgpu`：Windows DX12 / macOS Metal / Linux Vulkan）
- 字体：内嵌 JetBrains Mono（终端）+ Noto Sans SC Light（界面中文，细体）
- 终端仿真：`alacritty_terminal`
- SSH：双内核 `russh`（内置）+ 系统 `ssh`（`portable-pty`）
- 配置：YAML；凭据：系统 keyring + 加密 vault

## 当前进度

| 阶段 | 状态 |
|------|------|
| 1. 单连接 PTY + 终端渲染 | ✅ 骨架已通（本地 Shell） |
| 2. 多连接 + 竖排列表 | ✅ 基础 UI |
| 3. 会话树 + YAML 持久化 | ✅ 基础读写 + 演示数据 |
| 4. 双内核 + 认证 | 🚧 系统 OpenSSH 已接通；russh 内置待实现 |
| 5. 凭据加密 | 🚧 vault crate 已就位 |
| 6. 主题 / 快捷键 / 布局 | ⏳ |

## 仓库结构

```
vsterm/
├── assets/fonts/         # 内嵌字体（OFL）
├── crates/
│   ├── session-tree/
│   ├── vault/
│   ├── term-core/
│   ├── connection-mgr/
│   └── app-ui/           # egui 主程序 (bin: vsterm)
├── .github/workflows/    # CI + Release
└── Cargo.toml
```

## 环境要求

- Rust stable
- Windows：Visual Studio Build Tools（MSVC + Windows SDK）
- macOS：Xcode Command Line Tools
- Linux：`libxkbcommon-dev`、`libwayland-dev`、`libvulkan-dev` 等（见 CI）

## 构建与运行

```powershell
# Windows：建议先加载 VsDevCmd
cargo run -p app-ui
```

```bash
# macOS / Linux
cargo run -p app-ui
```

首次启动会在 `~/.vsterm/` 写入演示会话树。

## 发布（GitHub Actions）

推送版本 tag 后自动构建并上传三平台 64 位产物：

```bash
git tag v1.0.0
git push origin v1.0.0
```

产物示例：

- `vsterm-windows-x64.zip`
- `vsterm-macos-x64.tar.gz`
- `vsterm-macos-arm64.tar.gz`
- `vsterm-linux-x64.tar.gz`

也可在 Actions 里手动触发 `Release` 工作流（`workflow_dispatch`）打出 artifact。

> 仅 **推送 `v*` 标签** 或 **手动触发** 会运行 Release 构建；`main` 分支 push 不会触发。

## 数据目录

```
~/.vsterm/
├── sessions/tree.yaml
├── sessions/*.yaml
├── credentials/vault.enc
├── layouts/
├── themes/
└── config.yaml
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
