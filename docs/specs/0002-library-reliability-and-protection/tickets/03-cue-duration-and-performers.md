---
status: in-progress
---

# 03: CUE 时长与曲目演奏者完整呈现

**What to build:** Virtual track 按已知 CUE 边界显示时长，详情逐轨显示已解析演奏者；即使 DFF 标签不支持也不丢失 CUE 本身提供的资料。

**Blocked by:** 无（可立即开始）

**Status:** in-progress

- [x] 起点 0、终点 13500 帧的 Virtual track 显示 180000 ms，不依赖标签读取成功。
- [x] 末轨总长可知时显示剩余时长，不可知时如实显示未知。
- [x] 两首不同演奏者的曲目在详情各自显示正确演奏者。
- [x] 公开查询、数据库往返及应用重开后仍保留时长、演奏者和映像/CUE 来源。
- [x] 默认播放器入口准确说明打开整个映像，不引入定位播放、DFF 属性解析或切分文件。
- [ ] core 与状态层场景通过，并核对实际窗口中时长和演奏者确实被渲染；音乐与 CUE 内容不变。

**Demo:** 打开两轨 DFF 加 CUE，第一轨显示 3:00，末轨总长未知，两个演奏者分别可见；重开后不变。

**User Stories:** 48–54。

## 实施与验证记录

- 核心公开接缝 red：临时占位 DFF 加双轨 CUE 经实际 `DiskTags` 扫描、存储并重开后，时长原为 `[None, None]`，与规格的 `[Some(180000), None]` 不符。
- 核心 green：将已知 CUE 区间时长计算移至标签成功分支之外；同一场景保留两轨演奏者、映像/CUE 路径和边界，并核对音乐/CUE 内容及修改时间不变。既有已知总长的末轨回归仍通过。
- 状态层 red → green：增加重开后详情与播放器说明场景，新增公开按钮文案查询后通过；曲目行直接渲染已有的 `track.artist`。
- 定向验证：`voon-core --test album_detail`（3 项）、`voon-app --test album_detail --test player`；工作区 `cargo check --locked --offline`、格式和差异检查。并行 worktree 使用独立 target 重新验证，避免共享构建产物互相覆盖。
- 实际窗口及 Linux/Wayland 验收由总任务继续执行；当前未启动真实 Voon、未访问真实 NAS 或用户数据库，故状态保留 `in-progress`。

规格：[0002 资料库扫描可靠性与离线浏览](../../0002-library-reliability-and-protection.md)。通用约束与实施前沿见[工单索引](README.md)。
