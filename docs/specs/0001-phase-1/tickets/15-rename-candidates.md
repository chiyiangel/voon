---
status: ready-for-agent
---

# 15: 改名候选确认与忽略

**What to build:** 待处理中的"可能是改名/移动"条目显示旧路径 → 新路径，提供"确认"与"忽略"。确认后 Mobert 的字段编辑、User tag、Manual collection 成员资格一次性迁移到新路径，条目消失；忽略后条目不再出现。决定前再次改名时条目自动更新为"最初路径 → 最新路径"，改回原名时自动消失；内容完全相同的拷贝不会被配对。

**Blocked by:** 14 待处理视图；外部门槛：原型审核通过（PR #1 / https://voon-design-canvas.vercel.app）

**Status:** ready-for-agent

- [ ] 条目显示旧路径与新路径，以及"确认""忽略"按钮
- [ ] 确认后新路径的 Album 显示原有的编辑、User tag 和 Manual collection 成员资格，条目消失
- [ ] 忽略后条目消失且重扫不再出现
- [ ] voon-core 测试（NAS 形状假库）：改名并移动后出现候选；确认迁移三类用户数据；忽略；串接改名；改回原名；重复拷贝不配对（已有测试保留并扩充）
- [ ] 状态层测试：确认后待处理数量减一，新 Album 的详情状态带有迁移后的编辑
- [ ] 对应规格中的用户故事：90、91、92、93、94
- [ ] Linux 本地验证通过（build / clippy -D warnings / test --workspace）
- [ ] macOS 由 Mobert 手动验证

规格：[0001 第一期规格](../../0001-phase-1.md)
