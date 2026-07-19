# 性能基线 — 原生小窗（v2）

本仓库默认入口即为原生 `cat-desk-pet`（**无 WebView**）：小窗 (~180×180) + SVG/CPU 精灵 + 走路/闲置/睡觉 + 拖拽唤醒 + 点击穿透 + 托盘退出。

## 运行

```bash
cargo run --release
# 锁定姿态（测 CPU 用）
cargo run --release -- --mode sleeping   # sleeping | idle | walking
```

产物：`target/release/cat-desk-pet`

## CPU 对比口径

| 项 | 要求 |
|---|---|
| 工具 | `ps` 1Hz 采样 ≥30s |
| 包 | **release** / [v0.1.0](https://github.com/vvphp/desktop-cat/releases/tag/v0.1.0) 正式包 |
| 机器 | 同机、顺序测 |
| 记录 | 主进程；WebView 版 **加同 lifetime WebKit.GPU / WebContent** |

### 优化要点（v2）

- OS 窗位提交节流：漂移 ≥6px 且间隔 ≥80ms 才 `set_outer_position`
- 窗内像素偏移补偿，走路动画仍跟逻辑坐标
- walking 画帧 ~18fps；穿透轮询随模式降频

### 内存尖峰（issue #3）

- 可绘制窗边长硬顶 ~480；远距飞鸟/激光拖尾裁切，不再把主窗拉到接近全屏
- macOS `present_argb`：逻辑像素 + `contentsScale`，premull 双缓冲复用（无每帧 `Vec::collect`）
- 气泡字体改为打包 Noto Sans SC 子集 `assets/fonts/pet-ui.ttf`（~42KB，OFL），不再整文件加载 Arial Unicode（~22MB）

### 实测 — 2026-07-18 / Apple M5（v2）

warm 5s + 采样 30s。

#### native v2

| 场景 | avg %CPU | min–max |
|------|---------:|---------|
| sleeping | **1.36** | 0.9–2.2 |
| idle | **2.63** | 1.4–4.0 |
| walking | **5.53** | 4.9–6.6 |

#### v0.1.0（同轮复测）

| 口径 | avg %CPU |
|------|---------:|
| 主进程 + 同龄 WebKit.* | **17.37**（GPU≈7 + WebContent≈7 + main≈5） |

### 对比（上一轮 v1 → 本轮 v2）

| 场景 | spike v1 | spike v2 | v0.1.0+WebKit | v2 相对基线 |
|------|---------:|---------:|--------------:|------------|
| walking | 9.42 | **5.53** | 17.37 | **≈ −68%** |
| idle | 2.29 | 2.63 | — | 明显低于清醒基线 |
| sleeping | 1.35 | 1.36 | — | ≤2% 带 |

### Go / No-Go

**Go。** 清醒态（walking）相对 v0.1.0+WebKit **超过约 50% 降幅门槛**（实测 ≈ −68%）。  
可以进入 Phase 2（行为迁移）。

## Phase 2 进度（行为迁移，已完成）

行为已迁入原生状态机；旧 WebView 对照仓见 `vvphp/desktop-cat`。

| 块 | 状态 |
|---|---|
| 拖拽（`dragged`）+ 松手晕眩（`dizzy`） | ✅ |
| 抚摸（`pet`）长按不拖 | ✅ 按住 ≥500ms 且移动 <8px → 爱心眼；松手结束 |
| 撒娇（`clingy`） | ✅ 光标久不动自动靠近；托盘「💕 过来撒娇」 |
| 闲置子动作 sit / yawn / stretch / look / tail_curl | ✅ |
| 睡觉 + 简易床（`in_bed`） | ✅ 托盘：让她睡一下 / 回窝睡觉 |
| 回窝走路（`going_home`） | ✅ 「回窝睡觉」先走到右下角床位再 `in_bed`；途中床在家等着 |
| 追光标 interested / watching / chasing | ✅ |
| 玩具 / 喂食（投食、毛线球、弹力球、激光笔、逗猫棒） | ✅ 托盘触发；激光跟光标+拖尾，逗猫棒仅猫追 |
| 菜单 / 换毛色（橘/三花/奶牛/虎斑/黑白） | ✅ 托盘子菜单 |
| 鸟蝶 / 拍照（飞过、落鼻、惊吓、闪光） | ✅ 托盘 + 偶尔环境触发 |
| 物种（猫 / 猪 / 熊）+ 对应毛色与闲置特写 | ✅ 托盘：换宠物 / 换毛色；猪 `mud_roll`、熊靠边 `back_scratch`；投食 🐟/🥕/🍯 |
| 送礼（`gifting`） | ✅ 托盘/更多 + 偶尔自发；落叶/花/鼠/糖；放下后停留再淡出 |
| 菜单对齐（托盘 + 右键） | ✅ 结构对齐 WebView：🐾动物 / 🎨毛色 / 🎮互动 / 🧸玩具；右键点宠物弹出；纸团/假老鼠已补 |

## Phase 3 — 正式壳（当前默认）

| 项 | 状态 |
|---|---|
| 默认入口 `npm run dev` / `npm run build` → 原生 `cat-desk-pet` | ✅ |
| macOS `.app` / `.dmg`：`npm run package:macos` / `./tools/package-macos.sh` | ✅ |
| Release CI 编原生 universal + Windows exe | ✅ |
| README 架构与构建说明更新 | ✅ |

产物：`target/release/cat-desk-pet`  
打包：`dist/macos/摸鱼猫.app`、`摸鱼猫.dmg`

## 绘制路径（SVG）

身体用 WebView 原版 `CAT_SVG`（`assets/pet.svg`）经 **resvg** 栅格化后 blit；毛色 / 物种 / 眼口表情靠 CSS 变量替换 + 注入 `<style>`。腿/尾按 `walk_phase` / idle 动作量化后写入 SVG `transform`，缓存约 8 档关键帧。玩具、投食、鸟蝶、床等道具仍为程序化绘制。

### SVG 路径 CPU（2026-07-18 / Apple M5）

warm 3s + `ps` 1Hz × 20s；相对 v0.1.0+WebKit walking **17.37** 仍约 **−56%**（门槛 ≥50%）。

| 场景 | avg %CPU | min–max | 对比程序化简绘 v2 |
|------|---------:|---------|------------------|
| sleeping | **1.93** | 1.1–2.4 | 1.36 → 略升 |
| idle | **3.64** | 2.5–4.6 | 2.63 → 略升 |
| walking | **7.62** | 5.5–8.9 | 5.53 → 略升（首帧栅格缓存） |

## 已知限制

- 无整身倾斜 / 翻滚等复杂 idle 骨骼；腿尾为量化关键帧
- 透明/穿透依赖 macOS `NSWindow`；光标坐标按主屏原点换算（副屏 chase / laser 已按 primary 校正，极端多 DPI 布局仍建议手测）
- Windows 可编可跑，光标跟随/穿透完整度弱于 macOS（后续补）
