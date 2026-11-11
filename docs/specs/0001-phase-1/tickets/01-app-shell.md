---
status: in-progress
---

# 01: 应用骨架

**What to build:** 把 voon-app 从最小占位窗口换成按原型实现的三栏外壳：左侧导航（全部 Album、Collection、作曲家、Facet、User tag、Manual collection、待处理）、中间专辑区、右侧详情区，各区域先显示空态。同时建立应用状态层（view-model）：导航选中项、扫描状态、筛选与排序、选中的 Album 都由状态层持有，视图只读取状态、转发操作。搭好基于 GPUI 测试上下文的状态层测试脚手架，并做必要的预重构，让后续每个切片只需要"加状态 + 加视图 + 加测试"。界面文案全部为中文；默认 Library root 的平台差异保持现状。

**Blocked by:** None (can start immediately)；外部门槛：原型审核通过（PR #1 / https://voon-design-canvas.vercel.app）

**Status:** in-progress（实现与 macOS 自动检查已完成，平台与人工外观验收待办）

- [ ] 窗口打开后显示三栏布局，导航项、空态文案与原型一致（人工对照原型检查）
- [x] 导航选中项、选中的 Album 等界面状态全部由状态层持有，视图层不含业务逻辑
- [x] 状态层测试（GPUI 测试上下文）：切换导航项后状态层的选中项随之改变；初始状态为空库
- [x] 状态层测试脚手架可复用：后续工单能用同一套方式构造状态层并注入假数据
- [x] 界面中没有任何英文文案
- [ ] 对应规格中的用户故事：54、99、100、101、102
- [ ] Linux 本地验证通过（build / clippy -D warnings / test --workspace）
- [ ] macOS 由 Mobert 手动验证

规格：[0001 第一期规格](../../0001-phase-1.md)

## 实现与验证记录（2026-11-11）

- `voon-app` 分为 `platform`、GPUI Entity 状态层和 `ui`。导航宽 220、详情宽 340、工具栏高 52，浅色/深色 tokens 来自原型；窗口采用原生控制按钮。所有静态界面文案为中文，原型说明中的英文缩写改用中文；路径与音乐元数据不属于界面翻译文案。
- 状态层持有导航、扫描状态、搜索、格式筛选、排序、Album 列表及选择。`AppState::new(Index, suggested_root)` 接收本地 Index；测试以 `Index::open_in_memory()` 和 `store_scan` 注入假库，不访问 NAS，不打开原生窗口。
- TDD：依次添加空库、七导航、假 Index/Album 选择、搜索/筛选/排序保持四个 GPUI 测试。每轮先观察缺少模块或对应公开操作的编译失败，再添加最小实现，最终 `cargo test -p voon-app --test app_state --locked --offline` 为 4/4 通过。原有 root 优先级测试迁入 `platform.rs` 并保留通过。
- 本机 macOS：`cargo check --workspace --locked --offline` 通过；`cargo clippy --workspace --all-targets --locked --offline -- -D warnings` 通过；当时的 `cargo test --workspace --locked --offline` 共 42 项通过（含同时进行的 ticket 02 测试）。GPUI test-support 需要额外测试依赖，已更新 Cargo.lock。首次测试/lint 因 Metal 缓存沙箱权限失败，获准后重跑成功。
- 尚未完成 Linux 本地 build/lint/test/启动、Linux 千张假库滚动验收，以及 Mobert 的 macOS 人工外观验收；上述待办没有勾选。ticket 02 已复用同一构造与状态操作方式继续实现扫描。
