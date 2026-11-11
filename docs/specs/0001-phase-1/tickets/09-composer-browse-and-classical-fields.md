---
status: ready-for-agent
---

# 09: 作曲家浏览与古典字段

**What to build:** 详情中显示并支持作曲家、演奏者、指挥、乐团这几个古典字段（乐团在第一期只能由用户填写）。导航中有"作曲家"入口，列出所有作曲家及其 Album 数，点击后只显示该作曲家的 Album。

**Blocked by:** 06 专辑详情；外部门槛：原型审核通过（PR #1 / https://voon-design-canvas.vercel.app）

**Status:** ready-for-agent

- [ ] 详情显示作曲家、演奏者、指挥、乐团及其 Field source
- [ ] 导航中的作曲家列表按名称排序并显示 Album 数；点击后专辑区只显示该作曲家的 Album
- [ ] voon-core 测试：作曲家来自 Metadata tag 或 CUE；演奏者为各曲艺术家去重；按作曲家查询 Album
- [ ] 状态层测试：选中某作曲家后 Album 集合正确
- [ ] 对应规格中的用户故事：50、51、52、53
- [ ] Linux 本地验证通过（build / clippy -D warnings / test --workspace）
- [ ] macOS 由 Mobert 手动验证

规格：[0001 第一期规格](../../0001-phase-1.md)
