---
status: in-progress
---

# 0004 界面过渡动画盘点

把 Voon 一期桌面界面里目前「状态一变就硬切」的地方列清楚，并给出可在本机 GPUI 0.2.2 上落地的动画方案。本文件只做盘点与实施顺序，不包含动画代码。编号使用 0004，因为 0002 已被[资料库扫描可靠性与离线浏览](0002-library-reliability-and-protection.md)占用。

依据：Mobert 要求先盘点再改代码；Linux 由本机验证，macOS 由 Mobert 手工验证；不使用 Cursor Cloud Agent，不添加 GitHub Actions。

## 背景与目标

当前 `voon-app` 已经具备一期主界面：首次启动、扫描横幅、三栏外壳、Album 网格与列表、详情页、分面 / 作曲家 / 套装 / 标签与合集、待处理、编辑 / 选封面 / 合并拆分 / 整理对话框，以及独立设置窗口。代码里没有使用 `Animation`、`with_animation` 或任何过渡；`.hover()` / 选中态只是瞬时改样式。结果是切换视图、打开面板、出现横幅时都会突然跳变。

目标是在不改动一期信息架构的前提下，给关键状态变化补上符合 macOS 手感的短过渡：大约 150–250 ms，减速收尾。设计画布几乎没有动画，唯一写明的是扫描时新出现的封面卡片：

```css
.card.new .cover { animation: pop 0.6s ease-out; }
/* from { opacity: 0.2; transform: scale(0.96); } */
```

因此除「新 Album 入场」沿用 600 ms 的 pop 外，其余交互都按 150–250 ms 处理，不要做成展示型动效。

## 当前实现快照

盘点时仓库 `main` / 本分支起点为 `bbdd2bc15060b809d313030fc8de4cda5b759e3f`（2026-11-14 21:32，Asia/Shanghai）。远程另有 `design/prototype`（`95dc0b4`），[PR #1](https://github.com/chiyiangel/voon/pull/1) 仍未合并。无其他功能分支。

| 规格 | 代码 | 工单状态（以各 tickets 索引为准） |
|---|---|---|
| 0001 一期桌面功能 | 18 张票的界面与状态层已实现 | 全部 `in-progress`：macOS 本机测试已跑过，Linux 窗口验收与 Mobert 手工验收未关单 |
| 0002 扫描可靠性 | 功能代码已提交 | 01–04、06–10 为 `done`；05、11 为 `in-progress`（外部验收待办） |
| 0003 菜单与独立设置窗口 | 01–05 功能已接通 | 01 为 `done`；02–06 为 `in-progress`（Linux / 窗口证据待办） |

本机 Linux（X11，`DISPLAY=:3`）已成功编译并打开 `voon-app`。对照截图在仓库外的 `/workspace/voon-anim/before/`：`01-first-use.png`（尚未添加 Library root）、`02-album-grid.png`（对 `/workspace/voon-fake-library` 预扫描后的网格，6 张 Album，待处理 16 条）。未用按键或点击驱动界面，故列表、详情、对话框等其它帧以代码为准，不在本次截图里。

## GPUI 0.2.2 的能力与限制

依赖为 `gpui = "0.2"`，锁定 `0.2.2`。动画相关 API 在 `gpui::elements::animation`：

- `Animation::new(Duration)`，默认线性、只播放一次；`.repeat()` 循环；`.with_easing(fn)` 自定义曲线。
- 内置缓动：`linear`、`quadratic`、`ease_in_out`、`ease_out_quint()`（快入慢出，接近 macOS 的 ease-out）、`bounce(easing)`、`pulsating_between(min, max)`。也可以传入任意 `Fn(f32) -> f32`。
- `AnimationExt::with_animation(id, animation, |element, delta| …)`：在 0.0–1.0 的 `delta` 上改元素样式后返回。`with_animations` 可串联多段。
- 底层用 `window.request_animation_frame()` 在动画未结束时继续布局；`Window` / `Context` 还提供 `on_next_frame`。
- 任何实现了 `Styled` 的元素都可以动画 `opacity()`，也可以逐帧改颜色、宽高、边距、`relative` 宽度等。

限制（实施时必须遵守）：

1. **没有 CSS 那种隐式 transition。** `.hover()`、`.active()`、`.focus()` 只是条件样式，背景和边框会瞬间切换。要做悬停渐变，得自己听 `on_hover` 并驱动动画。
2. **没有布局自动插值（FLIP / layout animation）。** 网格改列数、列表高度变化、侧栏树展开，框架不会帮你补间。
3. **`Transformation`（scale / translate / rotate）只作用于 SVG**（`svg().with_transformation(...)`），普通 `div` / `img` 没有 transform。设计画布里封面 `scale(0.96 → 1)` 不能直接搬过来；可行做法是在固定尺寸的框里插值内层 `size()`，或先只做透明度。
4. **离场必须先留在树上。** `with_animation` 的状态按 `ElementId` 存。元素卸下再挂上，入场一般会重播；但如果对话框关闭时直接 `return div()`（现在编辑、封面、合并、整理都是这样），离场动画来不及播。需要显式的 closing 状态，播完再卸下。
5. **没有 Reduced Motion 接口。** 源码里搜不到 `prefers_reduced_motion` 或系统辅助功能查询。若要尊重系统设置，需要本项目自己读 OS API（macOS 的 `accessibilityDisplayShouldReduceMotion`，Linux 上至少尝试 `org.gnome.desktop.a11y.interface` / `prefers-reduced-motion`），读不到则提供设置项或默认开启动画。
6. **虚拟列表。** 网格和列表用 `uniform_list`。卡片入场可以对新出现的 path 做一次性动画；不要在每一行上挂循环动画。
7. **不要在动画里做重计算。** 分面栏宽度一旦按帧插值，会连带重算网格列数，容易抖动。分面 / 作曲家栏优先用透明度，而不是拉宽度。

动画基础设施放在 `crates/voon-app/src/motion.rs`（crate 根，而不是 `ui/motion.rs`），因为 `view_model` 不能依赖 `ui`，但关闭中的对话框需要 `Linger`。各视图只调用这些帮手，不复制动画样板。

## 时长约定

| 用途 | 时长 | 缓动 | 来源 |
|---|---|---|---|
| 对话框、菜单、横幅、视图切换、详情换页 | 180–220 ms | `ease_out_quint` | macOS 手感（任务给定的 150–250 ms） |
| 按钮按下、轻量选中反馈 | 80–120 ms | `ease_out_quint` | 更短，避免拖泥带水 |
| 扫描时新 Album 封面入场 | 600 ms | `ease_out_quint`（或等价 ease-out） | 设计画布 `.card.new` |
| 循环指示（封面加载、不确定进度） | 1–2 s 一圈 | `pulsating_between` 或 `repeat` | 仅用于进行中状态 |
| 系统开启 Reduced Motion 时 | 0 ms（直接到终态） | — | 辅助功能 |

## 盘点

优先级：P0 是现在最「硬」的、用户每次都会碰到的切换；P1 是常见但稍次的反馈；P2 是锦上添花，或受 API 限制不值得本轮强做。

| # | 位置 | 触发 | 建议动画 | 优先级 | 可行性 |
|---|---|---|---|---|---|
| 1 | `ui/edit_sheet.rs` 编辑信息遮罩与面板 | 打开 / 关闭编辑 Album | 遮罩透明度 0→0.4（`rgba(0x00000066)`），面板 opacity 0→1，220 ms，`ease_out_quint`；关闭反向并等结束再卸下 | P0 | 可行。需给 `EditSheet` 增加 closing 相位。面板不宜做 scale（div 无 transform），可用少量 `mt` 位移（约 8 px）模拟上浮，可选 |
| 2 | `ui/cover_picker.rs` 选择封面 | 同上 | 与编辑对话框同一套遮罩 + 面板入离场 | P0 | 同 1，`CoverPicker` 已有 `was`-类焦点逻辑，可沿用 |
| 3 | `ui/merge_sheet.rs` 合并 / 拆分 / 撤销确认 | 打开 / 关闭结构对话框 | 同上 | P0 | 同 1 |
| 4 | `ui/tags_view.rs` 整理对话框（新建 / 添加 / 删除标签或合集） | `organization_dialog()` 出现或消失 | 同上 | P0 | 同 1；现在无对话框时直接返回空 `div()` |
| 5 | `ui/application_menu_bar.rs` Linux 窗口菜单；`ui/toolbar.rs` 排序菜单 | 打开 / 关闭弹出菜单 | 入场 opacity 0→1，150–180 ms；离场同样需要短暂保留 | P0 | 可行。菜单是 `deferred` 绝对定位，只做透明度，不要改尺寸 |
| 6 | `ui/scan_banner.rs` 扫描状态行、NAS 离线横幅、`error()` 提示 | 横幅出现或消失（扫描开始 / 结束、目录离线或恢复、错误写入或清除） | 透明度 + 明确高度从 0 插到内容高度，200 ms，避免突然把网格顶下去 | P0 | 可行。现在无内容时仍渲染空的纵向 `div`，几乎不占位，出现时高度会跳。需在横幅自身记下上一次可见性。扫描没有确定总进度，本轮不新做百分比条；进行中可用一条细的不确定脉冲（见 18） |
| 7 | `ui/album_grid.rs` 卡片 / `ui/cover.rs` | 首次扫描或增量扫描中，新 Album 出现在网格 | 封面 opacity 0.2→1，内层尺寸 96%→100%（外框固定为 `edge`，避免撑乱网格），600 ms，`ease_out_quint`。只对「本轮新出现」的 path 播一次 | P0 | 部分可行。透明度没问题；scale 用内层 `size` 模拟，不要用 SVG `Transformation`。需要 view model 标记本轮新发现的 Album，或比较上一帧的 path 集合。`uniform_list` 可按 path 做 `ElementId`（现在已用 path 做卡片 id） |
| 8 | `ui/window.rs` 中间浏览区：网格 ↔ 列表；侧栏切换 音乐库 / 套装 / 作曲家 / 分面 / 标签 / 合集 / 待处理；首次添加目录后面板从空状态变成网格 | `view_mode()` 或 `navigation()` 变化，或 `roots()` 从空到非空 | 中间内容 opacity 交叉淡化 180–220 ms。分面、作曲家侧栏出现时也只做淡入，**不要**插值 `FACET_PANEL_WIDTH` / `COMPOSER_PANEL_WIDTH` | P0 | 可行。`AlbumGrid` / `AlbumList` 实体已常驻，现在是瞬间替换。交叉淡化要在外壳暂存「上一帧内容」或给两边同时挂 opacity。空状态 → 网格同理 |
| 9 | `ui/album_detail.rs` 右侧详情 | 选中另一张 Album，或从「选择一张专辑」到有内容 | 标题、封面和当前页内容 opacity 交叉淡化 180 ms。`ElementId` 必须包含 Album path，否则同一 id 不会重播 | P0 | 可行。`album_path` 变化时已经重置页内展开状态，正好作为动画起点 |
| 10 | `ui/facet_view.rs` 筛选芯片；网格结果随筛选变化 | 勾选或清除分面 | 芯片入场 / 离场淡化 150 ms；浏览区内容随筛选结果轻淡一次（复用 8） | P1 | 可行。芯片现在有则渲染、无则高度为 0，需要 closing 或至少淡入 |
| 11 | `ui/inbox_view.rs` 待处理列表 | 确认或忽略改名、条目因修复而消失 | 行 opacity 1→0，约 180 ms，然后再从数据中移除，避免列表突然塌缩 | P1 | 可行，但要在视图层保留「正在离开」的行，不能只听 `attention_items()` 瞬间缩短 |
| 12 | `ui/cover.rs` 封面从 Pending / Loading 到 Ready | 缩略图解码完成 | 图片 opacity 0→1，200 ms。Loading 可用很低存在感的呼吸透明度，1.2 s 循环；Reduced Motion 时静态占位即可 | P1 | 可行。`CoverStatus::Ready(image)` 出现时按封面身份做一次性淡入 |
| 13 | `ui/sidebar.rs` 导航选中；`collection_view.rs` / `composer_view.rs` / `settings_window.rs` 同类选中行 | 点击改变当前项 | 背景色在 `theme.window` / `well` / `selected` 之间插值 120–160 ms | P1 | 可行但要手写：记录上一次选中，用 `with_animation` 或自管时钟插值颜色。比透明度费事，不要和 `.hover()` 抢状态 |
| 14 | `ui/album_detail.rs` 概览 / 曲目 / 整理 / 文件页签；`overview.rs` / `tracks.rs` 元数据、来源、碟、曲目的展开 | 切换页签或展开合上区块 | 页签内容淡化 180 ms；展开区块只做内容淡入，不做自动高度（无 layout 动画） | P1 | 可行。页签已用不同 `scroll_id`。高度动画不建议做 |
| 15 | `ui/collection_view.rs` 套装树；侧栏「套装」chevron | 展开或折叠子节点 | 子行淡入 150 ms。整树高度不插值（`max_h(280)` 的树若做高度动画收益小、实现烦） | P1 | 淡入可行；高度动画列为不做 |
| 16 | `ui/settings_window.rs` 及四个子页 | 设置窗口内切换「音乐目录 / 扫描与读取 / 忽略规则 / 本地数据」 | 右侧页内容淡化 180 ms。窗口本身的出现交给系统窗口管理器，应用内不做 | P1 | 可行。页已经用 `("settings-page", page)` 作为 id |
| 17 | `ui/settings_window/local_data.rs` 封面缓存条 | `ready / total` 变化 | 前景宽度插值 200–300 ms，避免跳变 | P1 | 可行。用上一帧比例走到新比例；不要每帧重启动画 |
| 18 | `ui/scan_banner.rs` 扫描进行中（当前只有文字，没有条） | `ScanState::Scanning` | 可选：横幅底部一条 2–3 px 的不确定脉冲，使用 `pulsating_between` 或平移不了的透明度呼吸。不编造百分比 | P1 | 可行，且仅在有扫描任务时出现。若觉得多余可降为 P2 |
| 19 | 各处 `.hover()`：侧栏、列表行、工具栏按钮、设置按钮、菜单项 | 指针进入 / 离开 | 本轮默认不做颜色渐变。GPUI 的 hover 样式不能隐式插值，逐元素监听成本高，收益低于 P0 切换 | P2 | 技术上可行，不建议作为第一批 |
| 20 | 按钮按下态（现在几乎没有 `.active()`） | 鼠标按下 | 加 `.active(|s| s.opacity(0.85))` 即可，不必再套动画 | P2 | 完全可行，属于样式补齐 |
| 21 | 焦点环（`text_input.rs`、各对话框 `track_focus`） | 键盘聚焦 | 若补可见焦点环，用边框色或透明度在 100–150 ms 内出现。现在几乎没有可见 focus ring | P2 | 可行，但是视觉设计问题，先放着等原型或 Mobert 明确要求 |
| 22 | 从待处理跳到 Album（`AppState::open_attention`） | 点击待处理行 | 现在只 `select_album`，详情会变，但不会改 `navigation`，网格 / 列表里的 `UniformListScrollHandle` 也从未调用滚动。先实现滚动，再谈是否要平滑滚动；`uniform_list` 的平滑滚动能力有限，更可能是瞬间滚到目标行，再靠详情淡化（条目 9）给反馈 | P2 | 滚动本身尚未实现，不在本轮动画里先行设计复杂曲线 |
| 23 | Toast / 短暂成功反馈 | 保存编辑、合并成功等 | 产品里没有 toast，错误走横幅 `error()`（条目 6）。不要为动画新做一类浮层。若以后加 toast，再按 180 ms 淡入、停留、淡出 | P2 | 超出本次「补过渡」范围 |
| 24 | 系统外观切换（`Theme::for_window` 亮 / 暗） | `WindowAppearance` 变化 | 整窗颜色交叉淡化成本高，且少见，本轮不做 | P2 | 可行但收益低 |
| 25 | 设置窗口从菜单打开 | `open_window` | 交给 OS。Linux / XFCE 已有窗口出现方式；macOS 由系统处理 | P2 | 应用层不必做 |

## 推荐实施顺序

按用户可感知的「硬度」和共享基础设施排列，后一项尽量复用前一项的 `motion` 帮手。每一批都在 Linux 本机跑起来看过再交给 Mobert 在 macOS 上验收。

1. **基础设施。** 新增 `ui/motion.rs`：时长常量、缓动、入场包装、closing 相位、Reduced Motion 探测（读不到就先留接口，默认播放）。不改交互。
2. **P0 对话框与菜单。** 条目 1–5 共用同一套遮罩 / 面板 / 弹出层。这是目前最突然的打断。
3. **P0 横幅。** 条目 6（及可选的 18）：扫描、离线、错误的出现与收起，先消除纵向跳动。
4. **P0 浏览区切换与新卡片。** 条目 8、7、9：网格 ↔ 列表、侧栏换页、空状态到有库、新 Album 入场、详情换 Album。做完后，日常浏览的硬切应基本消失。
5. **P1 结果反馈。** 条目 10、11、12、14–17：筛选芯片、待处理行离开、封面加载、详情页签、套装树、设置页、缓存条。
6. **P1 选中色插值（条目 13）按余力决定。** 若与 hover 冲突就只做点击后的选中，不做悬停渐变。
7. **P2 一律不做，除非 Mobert 点名。** 悬停渐变、焦点环、toast、平滑滚动、窗口级外观过渡。按钮 `.active()` 透明度可以随手补上，工作量很小。

验收建议：

- Linux：对同一套假音乐库录切换前后的观感（对话框开合、网格 / 列表、扫描横幅、详情换张）。动画期间保持可点、可 Esc 关闭；Reduced Motion 开启时无播放、直接到终态。
- 现有两个测试切面（voon-core 公共接口、GPUI 状态层）不测像素或时长。动画是表现层，用状态机可测的部分仅限「关闭中的对话框在动画结束前仍算打开，结束后才为关闭」。
- macOS 由 Mobert 手工看：时长是否偏慢、字体与减运动设置是否生效。

## 明确不做

- 不为动画改一期信息架构，也不把设计画布里的示意数据带进产品。
- 不引入新的 UI 组件类型（toast、确定百分比扫描条）来「顺便」做动画。
- 不启动 Cursor Cloud Agent，不添加 `.github` workflow。
- 本文件在 `ui/transitions` 分支落地；合入 `main` 前须经 Mobert 看过盘点与优先级。


## 实施记录（in-progress）

本分支已在 Linux 落地下面这些条目。逻辑草稿（如 `album_edit_draft()`）仍然会立刻清空，以免破坏现有测试；视觉层通过 `*_view()` / `Linger` / `Leaving` 把内容留到离场结束。`VOON_REDUCE_MOTION` 可强制开关；为 `1` / `true` / `yes` / `on` 时所有时长变为 0，只保留透明度终态。未设置时：macOS 读取 `NSWorkspace.accessibilityDisplayShouldReduceMotion`（`cfg(target_os = "macos")`）；Linux 尽力执行 `gsettings get org.gnome.desktop.interface enable-animations`，`false` 视为减运动，没有 `gsettings` 则保持动画。

测试只加在状态层（GPUI `TestAppContext`）：对话框关闭后会先进入 closing，等时钟走完 overlay 时长再卸下；减运动时时长为 0、没有 closing 相位。没有像素测试。

| # | 优先级 | 结果 | 说明 |
|---|---|---|---|
| 1 编辑信息对话框 | P0 | 已做 | `album_edit_view()` + 遮罩/面板 `fade`，关闭待 overlay 完成 |
| 2 选择封面 | P0 | 已做 | 同一套 linger + fade |
| 3 合并 / 拆分对话框 | P0 | 已做 | 同一套 linger + fade |
| 4 整理对话框 | P0 | 已做 | 同一套 linger + fade；关闭中不抢焦点 |
| 5 菜单（应用菜单、排序菜单） | P0 | 已做 | 透明度入离场，菜单保持 mounted 到 menu 时长结束 |
| 6 扫描 / 离线 / 错误横幅 | P0 | 已做 | `ScanBanner` 实体，`reveal` 插值高度与透明度，避免网格被突然顶开 |
| 7 扫描中新卡片入场 | P0 | 已做 | 本进程扫描新出现的 path 记录 `Instant`；封面 opacity 0.2→1，内层尺寸 96%→100%，600 ms |
| 8 浏览区交叉淡化 | P0 | 已做 | 网格 ↔ 列表、待处理、首次使用、分面/作曲家栏只做 opacity，不插值宽度 |
| 9 详情换 Album | P0 | 已做 | 按 path 交叉淡化；上一张 Album 在详情实体里留到 detail 时长结束 |
| 10 分面芯片 | P1 | 已做 | 入场 / 离场淡化；逻辑筛选立刻生效 |
| 11 待处理行离开 | P1 | 已做 | `reload_library` 后消失的条目会留在视觉层淡出 |
| 12 封面 Ready 淡入 | P1 | 已做 | `CoverStatus::Ready` 按封面身份做一次 crossfade。Loading 没有另做呼吸，以免和扫描脉冲重复 |
| 13 侧栏选中色 | P1 | 已做 | 选中/取消选中用 selected 背景层做 opacity 淡化（等价于颜色过渡），不改 hover 瞬间样式 |
| 14 详情页签与展开 | P1 | 已做 | 页签内容淡入；字段来源、读取状态展开淡入。不做自动高度 |
| 15 套装树 | P1 | 已做 | 子行与整树收起淡化，高度不插值 |
| 16 设置分页 | P1 | 已做 | 右侧页内容交叉淡化；窗口出现仍交给系统 |
| 17 封面缓存条 | P1 | 已做 | 宽度按 progress 时长从上一帧比例插到新比例 |
| 18 扫描不确定脉冲 | P1 | 已做 | 仅在 `ScanState::Scanning` 时显示 3 px、1400 ms 透明度呼吸，不编造百分比 |
| 20 按钮按下透明度 | P2（顺手） | 已做 | 设置按钮、整理文字按钮、详情「播放 / 编辑」加了 `.active(|s| s.opacity(0.85))` |
| 19、21–25 | P2 | 跳过 | 按计划不做：悬停渐变、焦点环、待处理滚动、toast、外观切换、设置窗口系统级出现 |

macOS 请 Mobert 手工确认：系统「减少动态效果」与 `VOON_REDUCE_MOTION` 是否让时长归零；对话框 Esc / 点击关闭时面板是否播完离场再消失；网格滚动和多段动画同时播放时是否掉帧。
