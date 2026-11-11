# 第一期工单（0001 Phase 1 tickets）

由[第一期规格](../../0001-phase-1.md)拆分而来。每个工单是一个纵向切片：voon-core 公共接口 → 应用状态层 → GPUI 视图 → 两个已批准接缝上的测试。状态写在每个文件的 front matter 里（约定见[规格目录说明](../../README.md)）。

除 18 外，所有界面工单另有一个外部门槛：**原型审核通过（PR #1 / https://voon-design-canvas.vercel.app）**。

2026-11-11：Mobert 已在实现会话中确认「按现有原型开始实现」，外部门槛已满足。实现以本规格的范围为准，原型中的示意数据不进入产品。

| # | 标题 | Blocked by | Status |
|---|---|---|---|
| [01](01-app-shell.md) | 应用骨架 | — | in-progress |
| [02](02-library-roots-and-first-scan.md) | 根目录与首次扫描 | [01](01-app-shell.md) | in-progress |
| [03](03-album-grid.md) | 专辑网格 | [02](02-library-roots-and-first-scan.md) | in-progress |
| [04](04-incremental-rescan-and-offline-banner.md) | 增量重扫与 NAS 离线横幅 | [02](02-library-roots-and-first-scan.md) | in-progress |
| [05](05-list-view.md) | 列表视图 | [03](03-album-grid.md) | in-progress |
| [06](06-album-detail.md) | 专辑详情 | [03](03-album-grid.md) | in-progress |
| [07](07-collection-browse.md) | 合集浏览 | [03](03-album-grid.md) | in-progress |
| [08](08-facet-filters.md) | 分面筛选 | [03](03-album-grid.md) | in-progress |
| [09](09-composer-browse-and-classical-fields.md) | 作曲家浏览与古典字段 | [06](06-album-detail.md) | in-progress |
| [10](10-edit-album-fields.md) | 编辑专辑信息 | [06](06-album-detail.md) | in-progress |
| [11](11-choose-cover.md) | 选择封面 | [06](06-album-detail.md) | in-progress |
| [12](12-user-tags-and-manual-collections.md) | 用户标签与手动合集 | [06](06-album-detail.md) | in-progress |
| [13](13-open-in-default-player.md) | 用默认播放器打开 | [06](06-album-detail.md) | in-progress |
| [14](14-needs-attention-view.md) | 待处理视图 | [06](06-album-detail.md) | in-progress |
| [15](15-rename-candidates.md) | 改名候选确认与忽略 | [14](14-needs-attention-view.md) | in-progress |
| [16](16-merge-and-split-albums.md) | 合并与拆分专辑 | [10](10-edit-album-fields.md)、[14](14-needs-attention-view.md) | in-progress |
| [17](17-ignore-rules.md) | 设置中的忽略规则 | [02](02-library-roots-and-first-scan.md) | in-progress |
| [18](18-macos-packaging.md) | macOS 打包 | [03](03-album-grid.md) | in-progress |

## 按前沿推进（working the frontier）

可开工的工单 = 所有 Blocked by 都已是 `done` 的工单（以及外部门槛已满足）。完成一个工单后，把它的 front matter 和上表中的状态改为 `done`，再从新的前沿里挑下一个。同一前沿上的多个工单可以并行。开工时把状态改为 `in-progress`。

当前实现进度：18 张票的代码已实现并完成集成。macOS 本机全套 123 项测试、严格 Clippy、release 打包通过；Standards / Spec 两轴审查均无未解决项。实际窗口已验证扫描、浏览、编辑、封面、合并拆分及连续撤销。统一证据见[验收记录](../acceptance.md)。

本轮在 macOS 本机开发：已通过实现依赖的切片继续解锁后续编码；Linux 验收和 Mobert 手工验收尚未完成，因此工单保留 `in-progress`，不将代码实现等同于全部验收通过。
