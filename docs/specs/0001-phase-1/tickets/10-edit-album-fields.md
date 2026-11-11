---
status: in-progress
---

# 10: 编辑专辑信息

**What to build:** 在详情中编辑任意专辑级字段（标题、专辑艺术家、年份、流派、唱片编号、版本、作曲家、演奏者、指挥、乐团）。保存后该字段显示"我的编辑"来源标记，覆盖所有自动来源；可以一键恢复为自动值；重新扫描后编辑仍然生效。

**Blocked by:** 06 专辑详情；外部门槛：原型审核通过（PR #1 / https://voon-design-canvas.vercel.app）

**Status:** in-progress

- [ ] 编辑并保存后字段值立即更新，来源标记变为"我的编辑"
- [ ] "恢复自动值"后字段回到最强自动来源的值和标记
- [ ] 重扫后用户编辑仍然保留并生效
- [ ] voon-core 测试（NAS 形状假库）：设置用户值后查询结果来源为用户编辑；重扫后不变；清除后回到 Tag / CUE / 目录名中最强者
- [ ] 状态层测试：编辑草稿 → 保存 → 详情状态显示新值与用户编辑来源；恢复自动值后回退
- [ ] 对应规格中的用户故事：36、37、38、39、40、41、43、44、45
- [ ] Linux 本地验证通过（build / clippy -D warnings / test --workspace）
- [ ] macOS 由 Mobert 手动验证

规格：[0001 第一期规格](../../0001-phase-1.md)

## 实现与验证记录（2026-11-11）

- 详情提供“编辑信息”入口；弹层用真实 TextInput 编辑十个字段，支持保存、取消、关闭、Escape 与逐字段恢复自动值。未改动字段保留其原有来源；留空使用自动值。
- `Index::save_user_edits` 验证字段名及年份（1–9999），整张表单在一个本地事务中保存；任何非法值都不会部分写入。`automatic_metadata` 为草稿提供最强自动来源。
- 草稿固定到原 Album 路径。切换选中项或导航会废弃未保存草稿；保存失败时保留输入和错误，保存成功后列表及详情刷新，离线同样可编辑。
- `cargo test -p voon-core --test album_edits`：2 项通过，覆盖批量保存、重扫保留、恢复自动值、非法字段/年份整单拒绝。`cargo test -p voon-app --test album_edits`：2 项通过，覆盖草稿保存及恢复、校验错误、切换专辑防止误写。新增接口先观察编译失败；年份拒绝和换选中项先观察行为失败再实现。
- macOS `cargo check -p voon-app` 和定向 `cargo clippy -p voon-app --lib --bins --test album_edits -- -D warnings` 通过。Linux build / clippy / 完整测试，以及原生弹层输入、取消与 IME 的人工验收仍待完成，暂不标记 `done`。
