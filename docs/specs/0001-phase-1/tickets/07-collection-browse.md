---
status: ready-for-agent
---

# 07: 合集浏览

**What to build:** 左侧导航展开 Collection 层级（例如 Philips Duo、Alfred_Brendel，支持嵌套）。点击某个 Collection，专辑区只显示其中（含子 Collection）的 Album。

**Blocked by:** 03 专辑网格；外部门槛：原型审核通过（PR #1 / https://voon-design-canvas.vercel.app）

**Status:** ready-for-agent

- [ ] 导航中显示 Collection 树，嵌套层级正确
- [ ] 点击 Collection 后专辑区只显示该 Collection 下的 Album，并显示数量
- [ ] voon-core 测试（NAS 形状假库）：Collection 查询返回父子关系与其下 Album
- [ ] 状态层测试：选中 Collection 后过滤结果等于该 Collection 的 Album 集合
- [ ] 对应规格中的用户故事：20、21、58
- [ ] Linux 本地验证通过（build / clippy -D warnings / test --workspace）
- [ ] macOS 由 Mobert 手动验证

规格：[0001 第一期规格](../../0001-phase-1.md)
