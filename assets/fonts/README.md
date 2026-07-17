# Embedded fonts

| File | Source | License | When embedded |
|------|--------|---------|---------------|
| JetBrainsMono-Regular.ttf | https://github.com/JetBrains/JetBrainsMono | SIL OFL 1.1 | All platforms (terminal) |
| NotoSansSC-Light.otf | https://github.com/notofonts/noto-cjk (`Sans/SubsetOTF/SC`) | SIL OFL 1.1 | All platforms when system CJK is missing or too large |

UI CJK policy (RSS-aware):

- Prefer a **compact** system face when the file is ≤ the embedded subset (~8 MB).
- Otherwise use embedded Noto Sans SC Light instead of keeping YaHei / PingFang / full Noto CJK TTCs (often 11–40 MB) resident in the process heap.
- Never keep both a large system TTC and the embed at once.

Terminal uses **JetBrains Mono Regular** on every platform.
