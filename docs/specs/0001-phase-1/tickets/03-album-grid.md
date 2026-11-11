---
status: ready-for-agent
---

# 03: 专辑网格

**What to build:** 中间区域以封面网格展示索引中的 Album：封面、标题（已叠加用户编辑）、专辑艺术家，以及 Disc 数与 Track 数提示。封面按 voon-core 的自动规则选出；没有封面时显示占位图。几千张 Album 时滚动流畅（虚拟化列表）。点击某张 Album 时在状态层中选中它。

**Blocked by:** 02 根目录与首次扫描；外部门槛：原型审核通过（PR #1 / https://voon-design-canvas.vercel.app）

**Status:** ready-for-agent

- [ ] 网格显示每张 Album 的封面、标题、专辑艺术家，外观与原型一致（人工检查）
- [ ] 封面来自目录图片或内嵌封面；没有封面时显示占位图
- [ ] 上千张 Album 时滚动无明显卡顿（在 Linux 上用假库人工检查）
- [ ] 状态层测试：从索引加载后网格项数量等于 Album 数；点击某项后选中的 Album 改变
- [ ] voon-core 测试：索引查询返回的 Album 带有标题、专辑艺术家、Cover、Track 数
- [ ] 对应规格中的用户故事：55、59、60、85
- [ ] Linux 本地验证通过（build / clippy -D warnings / test --workspace）
- [ ] macOS 由 Mobert 手动验证

规格：[0001 第一期规格](../../0001-phase-1.md)
