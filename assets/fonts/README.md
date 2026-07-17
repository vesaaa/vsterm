# Embedded fonts

| File | Source | License | When embedded |
|------|--------|---------|---------------|
| JetBrainsMono-Regular.ttf | https://github.com/JetBrains/JetBrainsMono | SIL OFL 1.1 | All platforms (terminal) |
| NotoSansSC-Light.otf | https://github.com/notofonts/noto-cjk (`Sans/SubsetOTF/SC`) | SIL OFL 1.1 | **Linux only** (UI CJK fallback) |

On Windows / macOS the UI uses system CJK fonts (YaHei Light / PingFang SC, …).
On Linux, prefer system Noto CJK when present; otherwise use the embedded subset.
Terminal uses **JetBrains Mono Regular** on every platform.
