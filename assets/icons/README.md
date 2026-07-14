# VsTerm 图标资源包

## 目录结构

```
VsTerm-icons/
├── macos/
│   ├── VsTerm.icns          ← 直接用于 macOS 打包（.app 的 Info.plist 指向它）
│   └── icon_16/32/64/128/256/512/1024.png   ← 单独尺寸备用
├── windows/
│   ├── VsTerm.ico           ← 直接用于 Windows 打包（exe 图标 / .rc 资源文件）
│   └── icon_16/32/48/64/128/256.png
├── linux/
│   ├── 16x16/vsterm.png ~ 512x512/vsterm.png  ← 符合 freedesktop hicolor 图标规范目录结构
│   └── vsterm.svg           ← 矢量源文件，Linux 桌面环境优先使用 scalable 图标
├── web/
│   ├── favicon.ico / favicon-16x16.png / favicon-32x32.png / favicon-48x48.png
│   ├── apple-touch-icon.png        ← iOS "添加到主屏幕" 用，180x180，全出血无预制圆角
│   ├── android-chrome-192x192.png / android-chrome-512x512.png
│   └── site.webmanifest            ← PWA 场景用，非PWA可不引用
└── source/
    ├── icon_macos.svg / icon_macos_1024.png   ← macOS 风格源文件（渐变+光泽）
    ├── icon_flat.svg / icon_flat_1024.png     ← Windows/Linux 通用扁平风格源文件
    └── icon_apple_touch.svg                   ← apple-touch-icon 专用全出血源文件
```

## 各平台集成方式

### macOS（Rust + egui/eframe 打包场景）

如果用 `cargo-bundle` 或 `cargo-packager` 打包 `.app`：

```toml
# Cargo.toml（以 cargo-bundle 为例）
[package.metadata.bundle]
name = "VsTerm"
icon = ["macos/VsTerm.icns"]
```

如果是自己手写 `Info.plist`：

```xml
<key>CFBundleIconFile</key>
<string>VsTerm</string>
```

并将 `VsTerm.icns` 放入 `.app/Contents/Resources/` 目录下（去掉扩展名引用是 macOS 惯例）。

### Windows

**方式一：编译期嵌入（推荐，双击exe直接带图标）**

创建 `build.rs` 配合 `winres` 或 `embed-resource` crate：

```rust
// build.rs
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
        let mut res = winres::WindowsResource::new();
        res.set_icon("windows/VsTerm.ico");
        res.compile().unwrap();
    }
}
```

```toml
# Cargo.toml
[build-dependencies]
winres = "0.1"
```

**方式二：运行时设置窗口图标**（任务栏/标题栏，egui场景常用）

```rust
let icon_bytes = include_bytes!("../windows/icon_256.png");
let image = image::load_from_memory(icon_bytes)?.into_rgba8();
let (width, height) = image.dimensions();

let icon = eframe::egui::IconData {
    rgba: image.into_raw(),
    width,
    height,
};

eframe::NativeOptions {
    viewport: eframe::egui::ViewportBuilder::default().with_icon(icon),
    ..Default::default()
}
```

这段代码同样适用于 macOS/Linux，`egui::IconData` 是跨平台通用的窗口图标设置方式，建议三个平台统一走这条路径设置"运行时窗口图标"，而 `.icns`/`.ico` 主要用于"安装包/可执行文件本身"的图标（Finder、资源管理器里看到的图标）。

### Linux

按 freedesktop 图标规范，将 `linux/` 目录下内容安装到：

```
/usr/share/icons/hicolor/16x16/apps/vsterm.png
/usr/share/icons/hicolor/32x32/apps/vsterm.png
...
/usr/share/icons/hicolor/scalable/apps/vsterm.svg   ← 把 vsterm.svg 放这里
```

并在 `.desktop` 文件里引用：

```ini
[Desktop Entry]
Name=VsTerm
Icon=vsterm
Exec=vsterm
Type=Application
```

如果用 `cargo-deb` / `cargo-generate-rpm` 打包，通常在打包配置里指定资源文件映射到上述路径即可自动安装。

### Web（官网 / 文档站 favicon）

`web/` 目录下是给官网、文档站用的浏览器标签页图标和移动端主屏图标，在 HTML 的 `<head>` 里这样引用：

```html
<link rel="icon" type="image/x-icon" href="/favicon.ico">
<link rel="icon" type="image/png" sizes="16x16" href="/favicon-16x16.png">
<link rel="icon" type="image/png" sizes="32x32" href="/favicon-32x32.png">
<link rel="apple-touch-icon" sizes="180x180" href="/apple-touch-icon.png">
<link rel="manifest" href="/site.webmanifest">
<meta name="theme-color" content="#11182b">
```

其中 `apple-touch-icon.png` 用的是**不带预制圆角的全出血方形底图**（不是直接用 `icon_flat` 的圆角版本），这是苹果官方要求——iOS/iPadOS 会自动给"添加到主屏幕"的图标加圆角和高光效果，如果你自己预先做了圆角，会导致 iOS 二次裁切叠加出双重圆角、显得不协调。`favicon.ico`/`favicon-*.png` 场景则沿用之前的扁平圆角版本即可，因为浏览器标签页不会做二次裁切。

`site.webmanifest` 是给 PWA（渐进式网页应用，允许用户把网页"添加到主屏幕"当App用）场景用的，如果你的官网不需要 PWA 能力，这个文件可以不引用，不影响普通 favicon 效果。

## 关于这版设计的说明

- 主视觉是"V"字终端光标造型，末端叠加青绿/琥珀/珊瑚三色层叠方块，暗示多会话管理
- macOS 版本使用了柔和渐变背景 + 顶部高光，贴近 Big Sur 之后的系统图标"轻质感"风格；圆角比例做了较大幅度的 squircle 近似（非精确超椭圆路径）
- Windows/Linux 版本使用纯色扁平背景，圆角幅度更收敛，符合两个平台更简洁的图标语言

## 后续可优化项（当前为快速原型，非最终定稿）

1. **精确 squircle 曲线**：当前 macOS 版本用大圆角矩形近似，若要完全对齐苹果官方标准，建议用 [Apple Icon Composer](https://developer.apple.com/design/resources/) 或 Figma 的 macOS 图标模板重新描一版精确路径
2. **人工微调渐变/配色**：当前配色为快速验证版本，正式使用前建议设计师用 Illustrator/Figma 精修曲线和明暗过渡
3. **@2x/@3x 高分屏适配**：source 目录下的 SVG 可以随时按需重新导出任意分辨率，不受当前预设尺寸列表限制
