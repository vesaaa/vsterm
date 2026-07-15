# VsTerm 图标资源包 v3

> 本版更新：在 V 字光标与下方色块之间加入了明显间隔，避免二者视觉上黏连成一坨；
> 同时保留了 v2 版本「加粗线条、放大色块、收窄边距」带来的视觉分量优化，
> 解决了更早版本在 Windows 任务栏等小尺寸场景下「比同类应用图标显小」的问题。

## UI 线标（应用内控件）

会话树、文件列表、右键菜单等 UI 控件图标使用 [Lucide](https://lucide.dev/)（ISC 许可，扁平描线单色），经 [iconflow](https://crates.io/crates/iconflow) 以字体字形嵌入 egui。与下方品牌启动图标（ico/icns/png）相互独立。

## 目录结构

```
assets/icons/
├── macos/
│   ├── VsTerm.icns          ← macOS 打包（.app Info.plist 指向它）
│   ├── Info.plist           ← Release 打包用
│   └── icon_16…1024.png
├── windows/
│   ├── VsTerm.ico           ← Windows exe（build.rs / winres）
│   └── icon_16…256.png      ← 运行时窗口图标用 icon_256.png
├── linux/
│   ├── 16x16…512x512/vsterm.png
│   └── vsterm.svg
├── web/                     ← 官网 / 文档 favicon
├── source/                  ← SVG / 1024 源文件
├── app/window_icon.png      ← windows/icon_256.png 副本
└── vsterm.desktop
```

仓库根目录 `assets/branding/logo.png` 供 README / 文档展示。

## 设计迭代

| 版本 | 改动 |
|------|------|
| v1 | 初版：对角层叠色块，青紫深蓝配色探索 |
| v2 | 底部横向平铺色块；加粗线条、放大色块、收窄边距 |
| v3（当前） | V 字与色块之间约 9% 画布高度间隔，层次更清晰 |

## 接入点（本仓库已接好）

| 用途 | 路径 |
|------|------|
| Windows exe 嵌入 | `crates/app-ui/build.rs` → `windows/VsTerm.ico` |
| 运行时窗口/任务栏 | `crates/app-ui/src/icon.rs` → `windows/icon_256.png` |
| macOS `.app` | Release 工作流复制 `macos/VsTerm.icns` |
| Linux hicolor | Release 工作流安装 `linux/**/vsterm.png` + `vsterm.svg` |

Windows 替换图标后若任务栏仍显示旧图：重启资源管理器或清空图标缓存后再固定到任务栏。

## 各平台集成说明

详见历史备注：macOS 用 `CFBundleIconFile=VsTerm`；Windows `winres` + `egui::IconData`；Linux freedesktop `hicolor` + `Icon=vsterm`；Web 引用 `web/favicon*` 与 `apple-touch-icon.png`（全出血、无预制圆角）。
