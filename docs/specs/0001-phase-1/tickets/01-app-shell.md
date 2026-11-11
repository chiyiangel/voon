---
status: ready-for-agent
---

# 01: 应用骨架

**What to build:** 把 voon-app 从最小占位窗口换成按原型实现的三栏外壳：左侧导航（全部 Album、Collection、作曲家、Facet、User tag、Manual collection、待处理）、中间专辑区、右侧详情区，各区域先显示空态。同时建立应用状态层（view-model）：导航选中项、扫描状态、筛选与排序、选中的 Album 都由状态层持有，视图只读取状态、转发操作。搭好基于 GPUI 测试上下文的状态层测试脚手架，并做必要的预重构，让后续每个切片只需要"加状态 + 加视图 + 加测试"。界面文案全部为中文；默认 Library root 的平台差异保持现状。

**Blocked by:** None (can start immediately)；外部门槛：原型审核通过（PR #1 / https://voon-design-canvas.vercel.app）

**Status:** ready-for-agent

- [ ] 窗口打开后显示三栏布局，导航项、空态文案与原型一致（人工对照原型检查）
- [ ] 导航选中项、选中的 Album 等界面状态全部由状态层持有，视图层不含业务逻辑
- [ ] 状态层测试（GPUI 测试上下文）：切换导航项后状态层的选中项随之改变；初始状态为空库
- [ ] 状态层测试脚手架可复用：后续工单能用同一套方式构造状态层并注入假数据
- [ ] 界面中没有任何英文文案
- [ ] 对应规格中的用户故事：54、99、100、101、102
- [ ] Linux 本地验证通过（build / clippy -D warnings / test --workspace）
- [ ] macOS 由 Mobert 手动验证

规格：[0001 第一期规格](../../0001-phase-1.md)
