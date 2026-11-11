---
status: ready-for-agent
---

# 16: 合并与拆分专辑

**What to build:** 把被误分成多张的 Album 合并为一张（各自成为 Disc，可调整 Disc 顺序），或把一张 Album 拆成多张（按 Disc 或按选中的 Track）。操作可撤销，结果作为用户数据保存，重扫后保留；涉及的用户编辑按规则合并。需要在 voon-core 中新增合并 / 拆分 / 撤销接口及其持久化。

**Blocked by:** 10 编辑专辑信息、14 待处理视图；外部门槛：原型审核通过（PR #1 / https://voon-design-canvas.vercel.app）

**Status:** ready-for-agent

- [ ] 可多选 Album 合并，合并后显示为一张多碟 Album
- [ ] 可把一张 Album 按 Disc 或选中的 Track 拆成多张
- [ ] 合并、拆分都可撤销，撤销后恢复原状
- [ ] 重扫后合并、拆分结果保留
- [ ] voon-core 测试（NAS 形状假库）：合并两张 Album 后查询得到一张含两张 Disc 的 Album；拆分后得到两张；重扫后不变；撤销后恢复；Library root 中无文件变化
- [ ] 状态层测试：合并后专辑区数量减少、选中项为合并结果；撤销后恢复
- [ ] 对应规格中的用户故事：81、82、83、84
- [ ] Linux 本地验证通过（build / clippy -D warnings / test --workspace）
- [ ] macOS 由 Mobert 手动验证

规格：[0001 第一期规格](../../0001-phase-1.md)
