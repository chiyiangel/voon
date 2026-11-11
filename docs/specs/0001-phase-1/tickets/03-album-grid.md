---
status: in-progress
---

# 03: 专辑网格

**What to build:** 中间区域以封面网格展示索引中的 Album：封面、标题（已叠加用户编辑）、专辑艺术家，以及 Disc 数与 Track 数提示。封面按 voon-core 的自动规则选出；没有封面时显示占位图。几千张 Album 时滚动流畅（虚拟化列表）。点击某张 Album 时在状态层中选中它。

**Blocked by:** 02 根目录与首次扫描；外部门槛：原型审核通过（PR #1 / https://voon-design-canvas.vercel.app）

**Status:** in-progress（实现和 macOS 自动检查完成，人工外观/性能与平台验收待办）

- [ ] 网格显示每张 Album 的封面、标题、专辑艺术家，外观与原型一致（人工检查）
- [x] 封面来自目录图片或内嵌封面；没有封面时显示占位图
- [ ] 上千张 Album 时滚动无明显卡顿（在 Linux 上用假库人工检查）
- [x] 状态层测试：从索引加载后网格项数量等于 Album 数；点击某项后选中的 Album 改变
- [x] voon-core 测试：索引查询返回的 Album 带有标题、专辑艺术家、Cover、Track 数
- [ ] 对应规格中的用户故事：55、59、60、85
- [ ] Linux 本地验证通过（build / clippy -D warnings / test --workspace）
- [ ] macOS 由 Mobert 手动验证

规格：[0001 第一期规格](../../0001-phase-1.md)

## 实现与验证记录（2026-11-11）

- 中栏使用 GPUI `uniform_list` 按行虚拟化，只构造可见行；根据实际中栏宽度计算列数（最小封面宽 122、间距 16、边距 18）。卡片显示封面、叠加用户修改后的标题、专辑艺术家、Disc 数和 Track 数；点击把路径交给状态层选择。
- `AppState::grid_layout/grid_row` 提供同一网格状态接口。`AlbumGrid` 与详情区可共享 `CoverCache`，封面读取和解码在后台执行，最多四个任务并发、缓存最多 256 张且缩略图边长不超过 256。渲染期间不读取 NAS；缺失或解码失败显示「无封面」，读取中显示「正在加载」。
- `voon-core::tags::read_cover` 只读目录图片和音频内嵌封面（优先正面图，支持 DSF 的 ID3 与 Lofty 支持的格式）。`AlbumRow` 增加 `disc_count`，`from_album` 供扫描中的临时摘要复用。
- TDD：先观察 `disc_count`、`grid_layout/grid_row`、`read_cover`、`CoverCache/CoverStatus` 缺失导致的失败，再分片实现。`cargo test -p voon-core --test album_summary --locked --offline` 1/1；`cargo test -p voon-core cover_bytes_are_read --locked --offline` 1/1。审查后 `cargo test -p voon-app --test album_grid --locked --offline` 3/3：1003 张专辑在不同可用宽度下按排序顺序唯一可达，并可按路径选择；测试不约束像素或列数。封面测试验证缓存与库刷新后的重试、重新解码。
- macOS 的 `cargo check --workspace --locked --offline` 与 `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` 已通过。Linux 本地构建/运行、千张假库人工滚动、原型外观对照和 Mobert 的 macOS 人工验收尚未完成，未勾选相应项。
- 主代理随后通过临时 `.app` 的 macOS CUA smoke：添加 `/tmp/voon-acceptance/Music` 假库后出现 4 张网格卡片，中文/日文/英文音乐元数据正常，并显示双碟专辑的「2 碟 · 3 轨」。这是自动 UI 验证，不替代 Mobert 本人或 Linux 千张假库滚动验收。
