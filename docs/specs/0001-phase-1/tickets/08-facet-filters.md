---
status: in-progress
---

# 08: 分面筛选

**What to build:** 在导航或筛选栏中按 Facet 筛选：格式、分辨率等级（有损 / CD / Hi-Res / DSD64…）、最高采样率与位深、DSD / PCM、语种、年代，以及只来自 Metadata tag 的流派。多个 Facet 可以组合，每个取值显示对应 Album 数。

**Blocked by:** 03 专辑网格；外部门槛：原型审核通过（PR #1 / https://voon-design-canvas.vercel.app）

**Status:** in-progress（实现和 macOS 自动检查完成，平台/人工验收待办）

- [x] 可按格式、分辨率等级、采样率/位深、DSD/PCM、语种、年代、流派筛选，且可组合
- [x] 每个 Facet 取值旁显示 Album 数，随其他筛选条件更新
- [x] 流派只来自 Metadata tag，CUE 与目录名不产生流派
- [x] voon-core 测试（NAS 形状假库）：[DSD64] 目录得到 DSD64；[24-96] 得到 Hi-Res 与 96 kHz；中文标题得到中文语种；年份 1983 得到 1980 年代
- [x] 状态层测试：选择多个 Facet 后 Album 集合为交集，计数正确
- [ ] 对应规格中的用户故事：42、47、48、49、61、62、63、64、65、66、67、68
- [ ] Linux 本地验证通过（build / clippy -D warnings / test --workspace）
- [ ] macOS 由 Mobert 手动验证

规格：[0001 第一期规格](../../0001-phase-1.md)

## 实现与验证记录（2026-11-11）

- 236 宽的分面栏、可移除的已选条件、清除筛选和结果数已接入。状态层对同维度多选取并集、跨维度取交集；采样率/位深是下限。所有筛选与 Collection、搜索一起写入网格/列表共用的浏览缓存。
- `facet_options` 缓存在状态更新时计算，计数结合 Collection、搜索与其他筛选维度，排除自身维度；渲染不查数据库、不读 NAS。
- 自动流派仅取标签候选。Index 以一次批量查询读取原 Tag 流派，因此手动编辑显示值后，自动流派仍保持来源约束；CUE 和目录名不会贡献流派。
- TDD：公开 Facet 字段和状态 API 缺失先红，再实现；另外先观察手动编辑后原 Tag 流派丢失的断言失败，再修复批量读取。`cargo test -p voon-core --test facet_filters --locked --offline` 1/1；`cargo test -p voon-app --test facet_filters --locked --offline` 1/1（格式+规格、其他维度交集、同维并集、自身维度排除计数、清除）。
- macOS `cargo check --workspace --locked --offline` 与 `cargo clippy -p voon-app --lib --test facet_filters --test collection_browse --locked --offline -- -D warnings` 已通过。Linux 本地检查和 Mobert 手工验收待办，未勾选。
