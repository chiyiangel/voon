---
status: in-progress
---

# 09: 作曲家浏览与古典字段

**What to build:** 详情中显示并支持作曲家、演奏者、指挥、乐团这几个古典字段（乐团在第一期只能由用户填写）。导航中有"作曲家"入口，列出所有作曲家及其 Album 数，点击后只显示该作曲家的 Album。

**Blocked by:** 06 专辑详情；外部门槛：原型审核通过（PR #1 / https://voon-design-canvas.vercel.app）

**Status:** in-progress（实现与 macOS 自动检查完成，平台/人工验收待办）

- [x] 详情显示作曲家、演奏者、指挥、乐团及其 Field source
- [x] 导航中的作曲家列表按名称排序并显示 Album 数；点击后专辑区只显示该作曲家的 Album
- [x] voon-core 测试：作曲家来自 Metadata tag 或 CUE；演奏者为各曲艺术家去重；按作曲家查询 Album
- [x] 状态层测试：选中某作曲家后 Album 集合正确
- [ ] 对应规格中的用户故事：50、51、52、53
- [ ] Linux 本地验证通过（build / clippy -D warnings / test --workspace）
- [ ] macOS 由 Mobert 手动验证

规格：[0001 第一期规格](../../0001-phase-1.md)

## 实现与验证记录（2026-11-11）

- 作曲家面板按名称排序、显示数量，选择后与 Collection、搜索、Facet、组织筛选一起进入共享浏览缓存；网格/列表共用结果。状态层缓存选项和计数，界面不逐张查询数据库。回「音乐库」清除作曲家范围。
- 古典字段和来源复用 ticket 06 的详情，编辑复用 ticket 10；没有增加另一套字段。乐团没有自动来源。
- 公共核心测试先暴露缺少作曲家查询 API，以及 CUE 曲目演奏者被错误替换为整张 PERFORMER 的断言失败，再实现 `Index::albums_by_composer` 和 CUE 曲目演奏者按出现顺序去重；Tag 来源仍优先于 CUE。
- `cargo test -p voon-core --test composers --locked --offline` 1/1；`cargo test -p voon-app --test composer_browse --locked --offline` 1/1（名称排序、数量、选择结果、回全部），均先红后绿。`cargo clippy -p voon-app --lib --test composer_browse --locked --offline -- -D warnings` 通过。
- Linux 本地检查和 Mobert 的 macOS 人工验收仍待完成，未勾选。
