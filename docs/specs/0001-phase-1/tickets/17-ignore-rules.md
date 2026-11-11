---
status: in-progress
---

# 17: 设置中的忽略规则

**What to build:** 设置中显示内置忽略规则（.DS_Store、._ 开头文件、@eaDir、#recycle 等，以及"非音乐文件只识别封面"），并允许为每个 Library root 添加 / 移除要忽略的子文件夹。被忽略的子树在下次扫描时不再出现。需要在 voon-core 中新增按 Library root 保存用户忽略规则并在扫描中应用的接口。

**Blocked by:** 02 根目录与首次扫描；外部门槛：原型审核通过（PR #1 / https://voon-design-canvas.vercel.app）

**Status:** in-progress

- [x] 设置中列出内置忽略规则（只读）
- [x] 可为某个 Library root 添加、移除忽略的子文件夹
- [x] 添加忽略后重扫，该子树下的 Album 与 Collection 不再出现；移除后重扫恢复
- [x] voon-core 测试（NAS 形状假库）：配置忽略规则后扫描结果不含该子树；规则持久化并在重开索引后仍然生效
- [x] 状态层测试：添加忽略规则后触发重扫，Album 列表不再包含被忽略的 Album
- [ ] 对应规格中的用户故事：13、14
- [ ] Linux 本地验证通过（build / clippy -D warnings / test --workspace）
- [ ] macOS 由 Mobert 手动验证

规格：[0001 第一期规格](../../0001-phase-1.md)

实现证据（2026-11-11，macOS 开发机）：`voon-core/tests/ignore_rules.rs` 5 项、`voon-app/tests/ignore_rules.rs` 3 项通过。覆盖每根目录持久化、重开索引、Collection/Album 重扫隐藏及恢复、Disc 与附件封面过滤、拒绝根目录/路径穿越/目录外选择、离线移除规则；GPUI 状态测试使用真实文件后台扫描。设置提供只读内置列表及每根目录的子文件夹选择/移除，规则在下次重扫生效。对应测试目标的 clippy 和 app check 曾通过；未进行 Linux 或 Mobert 手工验证。
