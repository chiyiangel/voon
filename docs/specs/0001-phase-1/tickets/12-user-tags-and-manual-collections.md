---
status: in-progress
---

# 12: 用户标签与手动合集

**What to build:** 给一张或多张 Album 添加、移除 User tag；按 User tag 浏览；重命名、删除 User tag。创建 Manual collection，把 Album 加入、移除、调整顺序，重命名、删除 Manual collection；在导航中浏览。全部只保存在本机数据库，不写入任何文件。voon-core 需补齐重命名 / 删除 / 排序等接口。

**Blocked by:** 06 专辑详情；外部门槛：原型审核通过（PR #1 / https://voon-design-canvas.vercel.app）

**Status:** in-progress

- [x] 可对多选的 Album 批量添加、移除 User tag
- [x] 导航中显示 User tag 与 Manual collection，点击后显示对应 Album
- [x] Manual collection 内可调整顺序，顺序在重启后保留
- [x] User tag 与 Manual collection 可重命名和删除
- [x] voon-core 测试：标签与合集的增删改、排序；重扫后仍然存在；Library root 中没有新增文件
- [x] 状态层测试：批量打标签后按标签筛选得到这些 Album；合集排序后状态顺序正确
- [ ] 对应规格中的用户故事：69、70、71、72、73、74、75
- [ ] Linux 本地验证通过（build / clippy -D warnings / test --workspace）
- [ ] macOS 由 Mobert 手动验证

规格：[0001 第一期规格](../../0001-phase-1.md)

实现证据（2026-11-11，macOS 开发机）：`voon-core/tests/organization.rs` 4 项、`voon-app/tests/organization.rs` 5 项 GPUI 状态测试通过。覆盖批量加去标签、重命名/删除、去重、拒绝空名称和错误排序、合集顺序与成员跨重扫/重开保持、音乐库路径及文件内容不变；状态测试覆盖 modifier/Shift 多选、标签范围过滤、合集重排以及删除当前分类后刷新。核心排序以事务验证完整置换。界面提供侧栏分类、共享网格/列表缓存、批量操作栏、本地删除确认、合集上下移动及详情标签/合集信息；输入复用 UTF-16/IME 组件。core/app 的 organization 测试目标 clippy `--locked -- -D warnings` 与 app check 通过。尚未进行 Linux 验证或 Mobert 手工验收。

代理 macOS GUI 验证（2026-11-11）：给专辑添加验收标签后，侧栏对应计数为 1，详情同步显示标签 chip。另增加 GPUI 导航选择生命周期回归：切换导航或 Collection/Composer/User tag/Manual collection 范围时，同时清空主选中、多选与详情；网格/列表模式切换继续保留选择。此为代理验证，不代替 Mobert 手工验收。
