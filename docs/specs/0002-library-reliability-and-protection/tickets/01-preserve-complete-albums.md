---
status: done
---

# 01: 局部扫描失败保留完整 Album

**What to build:** 某个目录或 Disc 暂时不可读时，Voon 保留上次完整 Album，明确显示本次读取不完整；其他结构完整的 Album 仍可更新。首次发现但不完整的位置进入 Needs attention，不作为完整专辑发布。

**Blocked by:** 无（可立即开始）

**Status:** done

- [x] A 不可读、B 有正常变化时，A 保留全部 Disc、Track 和已知资料，B 的新增与真实移除正常更新。
- [x] 多碟 Album 缺一张 Disc 的观察时不发布缩水结构；目录失败与标签、封面失败的含义分开。
- [x] 首次发现即不完整的位置显示路径、阶段、原因与可查询的问题记录，普通列表不会把部分曲目当成完整 Album。
- [x] 问题状态经索引重开保留；后续完整重扫成功后更新资料并解除对应错误。
- [x] 列表、详情、Collection 导航和计数保持一致，人工编辑与路径关联的组织资料不因临时失败丢失。
- [x] 在 core 公共接口和 GPUI 状态层证明该链路，另以真实目录读取拒绝验证检测边界；不写入 Library root。

## 实现与验证记录（2026-11-11）

- 扫描记录成功列目录和不完整位置；目录项、stat 及 CUE 读取失败不再静默当作不存在。只有结构完整的 Album 进入后台逐张发现事件；首次失败仅进入待处理。
- Index 以已接受的物理 Album 快照保护失败范围，正常位置继续新增、更新及确认移除；完整基线与改名推断不接受不完整扫描。无新增数据库迁移。
- 待处理与详情显示“本次读取不完整”、路径、阶段、原因及保留旧资料状态；待处理可发起一轮后台重试。失败而无成功 Album 时不误进入空根确认。
- TDD red：真实 CD2 权限拒绝重扫由 2 碟缩水为 1 碟；不可读 CUE 首次误发布为单轨 Album；空 CUE 使 2 条 Virtual track 缩水；状态层缺少保留说明、重试接口及失败扫描误进入空根确认。对应 green 均通过。
- 正式验证使用独立 `CARGO_TARGET_DIR=/private/tmp/voon-0002-implementation/scan-target`，避免并行 worktree 产物混用：core `scan / scan_integrity / attention / library_roots / collections / album_structure` 共 33 项；GPUI `rescan / attention / album_structure / collection_browse` 共 13 项；`cargo check --workspace --locked --offline`、格式及差异检查通过。
- 真实读取拒绝由 macOS 临时目录/文件权限 `000` 产生，并断言操作系统返回 `PermissionDenied`，不是受控 Scanner 替代检测；测试结束恢复权限。未访问真实 NAS、用户数据库或启动真实窗口。实际窗口和 Linux/Wayland 验收交由工单 11，未声称通过。

**Demo:** 已有 A、B 两张专辑，令 A 或其 CD2 不可读，修改 B 后重扫；A 保留完整旧资料并显示错误，B 更新；重开后结果相同。

**User Stories:** 3、7–15、21。

规格：[0002 资料库扫描可靠性与离线浏览](../../0002-library-reliability-and-protection.md)。通用约束与实施前沿见[工单索引](README.md)。
