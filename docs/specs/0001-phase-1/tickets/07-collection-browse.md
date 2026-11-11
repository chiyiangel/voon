---
status: in-progress
---

# 07: 合集浏览

**What to build:** 左侧导航展开 Collection 层级（例如 Philips Duo、Alfred_Brendel，支持嵌套）。点击某个 Collection，专辑区只显示其中（含子 Collection）的 Album。

**Blocked by:** 03 专辑网格；外部门槛：原型审核通过（PR #1 / https://voon-design-canvas.vercel.app）

**Status:** in-progress（实现和 macOS 自动检查完成，平台/人工验收待办）

- [x] 导航中显示 Collection 树，嵌套层级正确
- [x] 点击 Collection 后专辑区只显示该 Collection 下的 Album，并显示数量
- [x] voon-core 测试（NAS 形状假库）：Collection 查询返回父子关系与其下 Album
- [x] 状态层测试：选中 Collection 后过滤结果等于该 Collection 的 Album 集合
- [ ] 对应规格中的用户故事：20、21、58
- [ ] Linux 本地验证通过（build / clippy -D warnings / test --workspace）
- [ ] macOS 由 Mobert 手动验证

规格：[0001 第一期规格](../../0001-phase-1.md)

## 实现与验证记录（2026-11-11）

- `Index::collection_tree` 返回显式父子关系和包含子孙 Collection 的成员；状态层维护展开路径、当前 Collection 和成员集合。侧栏树显示缩进、展开按钮及数量，中栏摘要显示当前实际结果数。
- 选择 Collection 只改变统一的 `browsed_albums` 缓存，与搜索、Facet 和网格/列表共用；点击「音乐库」清除 Collection 范围。无 NAS 读取。
- TDD：先观察 `collection_tree` 和公开状态操作缺失的失败，再实现。`cargo test -p voon-core --test collections --locked --offline` 1/1，`cargo test -p voon-app --test collection_browse --locked --offline` 1/1；假库验证嵌套父级 3 张、子级 2 张、全部 4 张，以及展开深度。
- macOS `cargo check --workspace --locked --offline` 与覆盖本票的 `cargo clippy -p voon-app --lib --test facet_filters --test collection_browse --locked --offline -- -D warnings` 已通过。Linux 本地检查与 Mobert 手工验收待办，未勾选。
