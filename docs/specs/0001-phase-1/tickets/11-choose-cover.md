---
status: ready-for-agent
---

# 11: 选择封面

**What to build:** 在详情中打开封面选择：列出该 Album 目录、Disc 目录、附件目录中的图片以及内嵌封面，选择其中一张作为 Cover；可以恢复为自动选择；选择在重扫后保留。需要在 voon-core 中新增：列出候选封面、保存用户选择的封面（作为用户数据，以 Album 路径为键）、清除选择。

**Blocked by:** 06 专辑详情；外部门槛：原型审核通过（PR #1 / https://voon-design-canvas.vercel.app）

**Status:** ready-for-agent

- [ ] 封面选择器列出所有候选图片和内嵌封面
- [ ] 选择后网格、列表、详情中的封面同步更新；恢复自动选择后回到自动规则的结果
- [ ] 重扫后用户选择的封面保留
- [ ] voon-core 测试（NAS 形状假库）：候选列表包含目录与 Booklet 中的图片；设置后查询得到用户封面；重扫后不变；清除后回到自动封面
- [ ] 状态层测试：选择封面后选中 Album 的封面状态改变；恢复后回退
- [ ] 对应规格中的用户故事：85、86、87
- [ ] Linux 本地验证通过（build / clippy -D warnings / test --workspace）
- [ ] macOS 由 Mobert 手动验证

规格：[0001 第一期规格](../../0001-phase-1.md)
