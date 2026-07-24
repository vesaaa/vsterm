# VsTerm

[English](README.md) | **简体中文**

**跨平台 SSH 终端管理工具**（Rust 原生）

集合 **WindTerm** / **Termius** / **FinalShell** 的优点：
专业终端体验 · 现代会话与凭据管理 · 文件传输与运维一体化

> **源代码已私有。** 本公开仓库仅保留产品说明与
> [二进制 Releases](https://github.com/vesaaa/vsterm/releases)。
> 后续开发在私有源码仓库进行。

## 下载

请从 **[Releases](https://github.com/vesaaa/vsterm/releases)** 下载对应平台安装包，解压后运行。
首次启动会创建 `~/.vsterm/`（含演示会话树）。

安装包尚未做微软 / 苹果公证，系统可能提示一次。

### Windows（SmartScreen）

1. 解压后双击 `VsTerm.exe`
2. 若被拦截：点 **更多信息** → **仍要运行**
3. 或：右键 → **属性** → 勾选 **解除锁定** → 确定

### macOS（无法打开 / 已损坏）

1. 按芯片选择：Apple Silicon 用 `vsterm-macos-arm64`，Intel 用 `vsterm-macos-x64`
2. **系统设置 → 隐私与安全性** → **仍要打开**
3. 若仍被隔离：

```bash
xattr -cr /path/to/VsTerm
```

### Linux

```bash
chmod +x VsTerm
./VsTerm
```

## 特色

- 终端 × SFTP 双向路径同步（含 sudo / 提权 SFTP）
- 会话树、YAML 配置、加密 vault
- 主机监控、路由表、网络连接、路径追踪
- ZMODEM（`rz`/`sz`）、本地 Shell、多标签重连
- 桌宠与连接特效

版本记录见 [CHANGELOG.md](CHANGELOG.md)。

## 授权声明

**保留所有权利（All Rights Reserved）**，并参考 CC BY-NC-ND 4.0 的非商业、禁止演绎精神。
完整条款见 [`LICENSE`](LICENSE)。

| 用途 | 是否允许 |
|------|----------|
| 使用官方发布的安装包（个人） | ✅ |
| 修改 / fork / 二次打包发布 | ❌ 未经书面授权禁止 |
| 魔改后主张知识产权或 rebranding | ❌ 未经书面授权禁止 |
| 集成到其他项目 | ❌ 未经书面授权禁止 |
| 商业盈利 / 企业生产 / 商业交付 | ❌ 须事先购买商业授权 |

**商业授权：** [vesaazheng@gmail.com](mailto:vesaazheng@gmail.com)

> 二进制中的第三方库与内嵌字体仍受其各自许可证约束。
