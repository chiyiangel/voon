---
status: ready-for-agent
---

# 12: 用户标签与手动合集

**What to build:** 给一张或多张 Album 添加、移除 User tag；按 User tag 浏览；重命名、删除 User tag。创建 Manual collection，把 Album 加入、移除、调整顺序，重命名、删除 Manual collection；在导航中浏览。全部只保存在本机数据库，不写入任何文件。voon-core 需补齐重命名 / 删除 / 排序等接口。

**Blocked by:** 06 专辑详情；外部门槛：原型审核通过（PR #1 / https://voon-design-canvas.vercel.app）

**Status:** ready-for-agent

- [ ] 可对多选的 Album 批量添加、移除 User tag
- [ ] 导航中显示 User tag 与 Manual collection，点击后显示对应 Album
- [ ] Manual collection 内可调整顺序，顺序在重启后保留
- [ ] User tag 与 Manual collection 可重命名和删除
- [ ] voon-core 测试：标签与合集的增删改、排序；重扫后仍然存在；Library root 中没有新增文件
- [ ] 状态层测试：批量打标签后按标签筛选得到这些 Album；合集排序后状态顺序正确
- [ ] 对应规格中的用户故事：69、70、71、72、73、74、75
- [ ] Linux 本地验证通过（build / clippy -D warnings / test --workspace）
- [ ] macOS 由 Mobert 手动验证

规格：[0001 第一期规格](../../0001-phase-1.md)
