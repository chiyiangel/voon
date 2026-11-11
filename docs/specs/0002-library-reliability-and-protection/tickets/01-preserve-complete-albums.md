---
status: ready-for-agent
---

# 01: 局部扫描失败保留完整 Album

**What to build:** 某个目录或 Disc 暂时不可读时，Voon 保留上次完整 Album，明确显示本次读取不完整；其他结构完整的 Album 仍可更新。首次发现但不完整的位置进入 Needs attention，不作为完整专辑发布。

**Blocked by:** 无（可立即开始）

**Status:** ready-for-agent

- [ ] A 不可读、B 有正常变化时，A 保留全部 Disc、Track 和已知资料，B 的新增与真实移除正常更新。
- [ ] 多碟 Album 缺一张 Disc 的观察时不发布缩水结构；目录失败与标签、封面失败的含义分开。
- [ ] 首次发现即不完整的位置显示路径、阶段、原因与可查询的问题记录，普通列表不会把部分曲目当成完整 Album。
- [ ] 问题状态经索引重开保留；后续完整重扫成功后更新资料并解除对应错误。
- [ ] 列表、详情、Collection 导航和计数保持一致，人工编辑与路径关联的组织资料不因临时失败丢失。
- [ ] 在 core 公共接口和 GPUI 状态层证明该链路，另以真实目录读取拒绝验证检测边界；不写入 Library root。

**Demo:** 已有 A、B 两张专辑，令 A 或其 CD2 不可读，修改 B 后重扫；A 保留完整旧资料并显示错误，B 更新；重开后结果相同。

**User Stories:** 3、7–15、21。

规格：[0002 资料库扫描可靠性与离线浏览](../../0002-library-reliability-and-protection.md)。通用约束与实施前沿见[工单索引](README.md)。
