# Embedded fonts

| File | Source | License |
|------|--------|---------|
| JetBrainsMono-Regular.ttf / Bold.ttf | https://github.com/JetBrains/JetBrainsMono | SIL OFL 1.1 |
| NotoSansSC-Regular.otf | https://github.com/notofonts/noto-cjk (`Sans/SubsetOTF/SC`) | SIL OFL 1.1 |

Embedded into the VsTerm binary at compile time so UI Chinese and
terminal Latin text render without depending on system fonts.

`NotoSansSC-Regular.otf` is the official Simplified Chinese regional subset
(~8 MB). Full Pan-CJK is not bundled.
