---
status: done
---

# 08: 标题与年份的完整来源比较

**What to build:** 先为标题与年份交付完整的标签、CUE、目录候选链：从扫描保存具体来源和观察时间，到详情比较、人工覆盖和恢复自动值，重开后仍可使用。

**Blocked by:** [01 局部扫描失败保留完整 Album](01-preserve-complete-albums.md)；[02 标签失败分类与增量恢复](02-recover-tag-reads.md)

**Status:** done

- [x] 标题与年份在入库前不丢弃同来源不同文件的候选，保留源实例、适用范围和真实观察时间。
- [x] 详情可比较各候选、采用值、自动值及人工值；来源可对应具体文件、CUE 或目录。
- [x] UserEdit > Tag > Cue > FolderName 不变，来源内部继续沿用既有确定性规则。
- [x] 人工编辑经重扫保留，恢复自动值不删除候选；空白沿用恢复自动值的现有语义。
- [x] 来源失败时保留上次成功证据和可理解状态，缓存命中不伪造新读取时间。
- [x] 数据库及应用重开后离线可查，完整 Album 发布与候选一致，不要求旧库迁移。
- [x] core 公共查询和 GPUI 状态验证从源值到界面的整条链，不单独交付没有 UI 的候选表。

**Demo:** 标签年份 1999、CUE 年份 1988、目录年份另有信息，详情能比较全部来源；手动修改、重扫、恢复自动再重开，候选仍在。

**User Stories:** 33–41、44、45、47（本票以标题、年份为首个字段范围）。

规格：[0002 资料库扫描可靠性与离线浏览](../../0002-library-reliability-and-protection.md)。通用约束与实施前沿见[工单索引](README.md)。

## 实施与验证（2026-11-11）

- Album 保存具体候选、源路径、整文件或 CUE 区间范围、真实读取时间及确认状态；原自动解析策略保持不变。
- Index.metadata_comparison 与 AppState.album_metadata_comparison 返回采用值、自动值、人工值和全部候选；详情的“比较字段来源”使用同一读模型。
- 首个 core 回归先因缺失公开比较接口失败，再验证两个标签文件、CUE、目录的年份 1977/1988/1999/2001 全部保留；断开临时库目录、重开及恢复自动值后仍可查询。失败来源保留旧时间，另一文件保持成功状态。
- 定向 core candidates 2 项、album_detail 3 项、album_structure 12 项及 GPUI candidates 1 项、attention 3 项、album_edits 2 项通过；workspace check、fmt、diff-check 通过。
- 实际窗口中的长路径可读性在工单 11 继续核对，不把状态层查询当作截图证据。
