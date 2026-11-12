---
status: in-progress
---

# 0002 资料库扫描可靠性与离线浏览工单

由[0002 规格](../../0002-library-reliability-and-protection.md)拆分。用户已确认以下 11 张工单的颗粒度与依赖，现按[项目 tracker 约定](../../README.md)一票一文件发布。每票状态由 front matter 和 Status 字段共同记录，依赖在各票的 Blocked by 中给出；表中编号均属于本规格。

## 工单与依赖

| # | 标题 | Blocked by | Status |
| --- | --- | --- | --- |
| [01](01-preserve-complete-albums.md) | 局部扫描失败保留完整 Album | — | done |
| [02](02-recover-tag-reads.md) | 标签失败分类与增量恢复 | — | done |
| [03](03-cue-duration-and-performers.md) | CUE 时长与曲目演奏者完整呈现 | — | done |
| [04](04-persistent-cover-thumbnails.md) | 当前 Cover 持久化与离线重开 | — | done |
| [05](05-new-index-and-version-guard.md) | 新建索引与资料库版本保护 | — | in-progress |
| [06](06-scan-interruption-and-organization.md) | 取消离线与结构整理保持一致 | [01](01-preserve-complete-albums.md) | done |
| [07](07-retry-and-force-reread.md) | 重试失败项与强制重读标签 | [01](01-preserve-complete-albums.md)、[02](02-recover-tag-reads.md) | done |
| [08](08-title-and-year-candidates.md) | 标题与年份的完整来源比较 | [01](01-preserve-complete-albums.md)、[02](02-recover-tag-reads.md) | done |
| [09](09-all-field-candidates.md) | 全部现有字段的候选与编辑一致性 | [08](08-title-and-year-candidates.md) | done |
| [10](10-library-cover-prefetch.md) | 全库封面预缓存与失败重试 | [04](04-persistent-cover-thumbnails.md) | done |
| [11](11-end-to-end-acceptance.md) | 故障交错与真实样本联验 | [03](03-cue-duration-and-performers.md)、[05](05-new-index-and-version-guard.md)、[06](06-scan-interruption-and-organization.md)、[07](07-retry-and-force-reread.md)、[09](09-all-field-candidates.md)、[10](10-library-cover-prefetch.md) | in-progress |

## 依赖执行记录

初始前沿 **01、02、03、04、05** 没有前置工单，可以独立开始。其余票仅在 Blocked by 的票全部完成后进入前沿；ready-for-agent 表示工单已准备好，不免除依赖条件。

- 01 完成后，06 可开始。
- 01、02 完成后，07、08 可开始。
- 08 完成后，09 可开始。
- 04 完成后，10 可开始。
- 03、05、06、07、09、10 全部完成后，11 可开始；其他依赖通过这些末端传递，不重复列出。

## 通用交付约束

- 每张功能票都是包含持久化、公开接口、应用状态、界面和验证的完整纵向切片。局部模块整理在对应切片开始时完成，不能以只建表、只改接口或只加按钮宣告完成。
- 沿用两个已批准测试接缝：voon-core 公共接口和 GPUI 应用状态层。测试断言外部可观察行为，不新增私有函数、SQL 或像素布局接缝。
- 每票完成自己的定向回归和约定的可见行为验证；11 负责交错场景、真实样本和平台联验，不代替前面各票的测试。
- 本轮从新建索引扫描开始，保留已有旧库文件。Library root 始终只读，本地产物不进入音乐目录。
- 备份、恢复、轮换、旧库迁移、在线资料、Version group 与 Copy 管理均不在本轮范围；实施延期的 ADR 不构成依赖。
- 04 先让已经请求的当前 Cover 可持久使用，10 再实现未浏览 Album 的全库覆盖；08 先让标题和年份完整可比较，09 再覆盖全部现有字段。各阶段都应能独立演示。
- 同时处理共享代码时需协调变更与集成，但共同修改同一位置本身不增加 Blocked by。
- 开工时将对应工单与本索引状态更新为 in-progress，完成约定验收后更新为 done；未验证的平台或窗口结果如实记录，不以代码已实现代替验收。

## 最终外部验收条件

11 使用约 20 张用户认可的真实专辑；真实样本和读取位置在执行前确定。真实 NAS、实际窗口及相应平台环境可用后完成对应验收，不虚报结果，也不据此阻塞无外部依赖的功能票开工。

2026-11-11 已按用户指令进入实施；功能票验收与最终外部验收分别记录，未验证的环境不计为通过。

实施代码已提交；目前 05 与 11 保留外部验收待办，其余功能票已完成约定实现与验证。实际证据、环境中断及未确认项见[联验记录](../acceptance.md)，不以临时目录中的未确认结果计为通过。
