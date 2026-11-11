---
status: ready-for-agent
---

# 08: 分面筛选

**What to build:** 在导航或筛选栏中按 Facet 筛选：格式、分辨率等级（有损 / CD / Hi-Res / DSD64…）、最高采样率与位深、DSD / PCM、语种、年代，以及只来自 Metadata tag 的流派。多个 Facet 可以组合，每个取值显示对应 Album 数。

**Blocked by:** 03 专辑网格；外部门槛：原型审核通过（PR #1 / https://voon-design-canvas.vercel.app）

**Status:** ready-for-agent

- [ ] 可按格式、分辨率等级、采样率/位深、DSD/PCM、语种、年代、流派筛选，且可组合
- [ ] 每个 Facet 取值旁显示 Album 数，随其他筛选条件更新
- [ ] 流派只来自 Metadata tag，CUE 与目录名不产生流派
- [ ] voon-core 测试（NAS 形状假库）：[DSD64] 目录得到 DSD64；[24-96] 得到 Hi-Res 与 96 kHz；中文标题得到中文语种；年份 1983 得到 1980 年代
- [ ] 状态层测试：选择多个 Facet 后 Album 集合为交集，计数正确
- [ ] 对应规格中的用户故事：42、47、48、49、61、62、63、64、65、66、67、68
- [ ] Linux 本地验证通过（build / clippy -D warnings / test --workspace）
- [ ] macOS 由 Mobert 手动验证

规格：[0001 第一期规格](../../0001-phase-1.md)
