---
status: in-progress
---

# 02: 根目录与首次扫描

**What to build:** 在设置中添加、移除 Library root。添加后在后台开始首次扫描：Album 一张张出现，显示当前目录和已发现数量，可以随时取消，取消后保留已找到的部分。扫描结果写入本机索引，重启应用后仍在。移除 Library root 时明确告诉 Mobert 他的编辑（字段编辑、User tag、Manual collection 成员资格，均以 Album 路径为键）是否会保留。扫描对 NAS 只读，杂项和非音乐文件被忽略，Album / Disc / Collection / Virtual track 的识别沿用 voon-core 现有规则。

**Blocked by:** 01 应用骨架；外部门槛：原型审核通过（PR #1 / https://voon-design-canvas.vercel.app）

**Status:** in-progress

- [ ] 设置中可以添加多个 Library root，也可以移除；移除时弹出提示，说明用户编辑是否保留
- [ ] 扫描在后台线程运行，扫描期间界面可操作；进度显示当前目录与已发现的 Album 数
- [ ] Album 随扫描逐张出现；取消后扫描停止，已找到的 Album 保留并标明"已取消"
- [ ] 重启应用后，上次的扫描结果从本机索引加载，无需重扫
- [ ] voon-core 测试（NAS 形状假库）：带进度回调的扫描逐张报告 Album；取消后返回部分结果并标记已取消；索引按 Library root 整体替换；移除 Library root 后其 Album 不再出现
- [ ] 状态层测试：添加 Library root → 扫描中状态 → 逐张收到 Album → 完成；扫描中取消 → 已取消状态且保留已有 Album
- [ ] 扫描过程不在 Library root 中创建、修改或删除任何文件（测试中对比扫描前后的目录内容）
- [ ] 对应规格中的用户故事：1、2、3、4、5、6、7、10、13、15、16、17、18、19、20、23、24、25、26、28、29、30、31、32、35、104、105
- [ ] Linux 本地验证通过（build / clippy -D warnings / test --workspace）
- [ ] macOS 由 Mobert 手动验证

规格：[0001 第一期规格](../../0001-phase-1.md)

## 实现与验证记录（2026-11-11）

- 已接通本机 SQLite、根目录添加/移除确认、后台扫描事件、逐张呈现与取消后的部分结果保存。
- 移除根目录保留字段编辑、User tag 和 Manual collection 成员关系，重新添加相同路径后恢复。
- `library_roots` 公共接口测试 2 项通过；`library_scan` GPUI 状态层测试 3 项通过，包含防止音乐库覆盖本地索引的只读约束。新增接口已先观察失败再实现。
- macOS `cargo check --workspace --locked --offline` 通过。Linux 验证、原生界面验收和 Mobert 手工验证仍待完成，暂不标记 `done`。
