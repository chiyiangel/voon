---
status: in-progress
---

# 11: 选择封面

**What to build:** 在详情中打开封面选择：列出该 Album 目录、Disc 目录、附件目录中的图片以及内嵌封面，选择其中一张作为 Cover；可以恢复为自动选择；选择在重扫后保留。需要在 voon-core 中新增：列出候选封面、保存用户选择的封面（作为用户数据，以 Album 路径为键）、清除选择。

**Blocked by:** 06 专辑详情；外部门槛：原型审核通过（PR #1 / https://voon-design-canvas.vercel.app）

**Status:** in-progress（实现与 macOS 自动验证完成，平台/人工验收待办）

- [x] 封面选择器列出所有候选图片和内嵌封面
- [x] 选择后网格、列表、详情中的封面同步更新；恢复自动选择后回到自动规则的结果
- [x] 重扫后用户选择的封面保留
- [x] voon-core 测试（NAS 形状假库）：候选列表包含目录与 Booklet 中的图片；设置后查询得到用户封面；重扫后不变；清除后回到自动封面
- [x] 状态层测试：选择封面后选中 Album 的封面状态改变；恢复后回退
- [ ] 对应规格中的用户故事：85、86、87
- [ ] Linux 本地验证通过（build / clippy -D warnings / test --workspace）
- [ ] macOS 由 Mobert 手动验证

规格：[0001 第一期规格](../../0001-phase-1.md)

## 实现与验证记录（2026-11-11）

- 详情封面可点击打开选择器。候选包含专辑目录、Disc 目录、附件子树和各音频文件的全部内嵌图片；同一 Whole-disc image 只枚举一次。`EmbeddedPicture { path, index }` 精确表示不同内嵌图，原 `Embedded(path)` 保留自动优先正面的语义。
- 候选枚举、图片读取和解码均在后台进行；候选按行虚拟化，复用网格/详情/列表的有界缩略图缓存。草稿在「应用」前不落库，「取消」或 Escape 不改变 Cover，「自动选择」移除用户覆盖。
- 本地 `user_cover` 以 Album 路径持久化，summary/detail 统一叠加；重扫保留用户选择。确认 Rename candidate 时在同一事务内迁移选择并调整封面路径前缀。事务友好的读/写 helper 供 ticket 16 的合并/拆分撤销复用，所有音乐与图片文件只读。
- TDD：先观察候选/持久化 API 缺失失败，再实现；另先观察确认改名后用户封面丢失的断言失败，再补事务迁移。`cargo test -p voon-core --test covers --locked --offline` 2/2；`cargo test -p voon-app --test cover_picker --locked --offline` 1/1（草稿、取消、应用同步、恢复自动）。本票与 01/03/07/08/09 的定向组合回归共 15 项通过。
- macOS `cargo check --workspace --locked --offline` 与 `cargo clippy -p voon-app --lib --test cover_picker --locked --offline -- -D warnings` 通过。
- 主代理 macOS CUA smoke：详情打开选择器，显示 `Booklet/alternate.png` 与 `cover.png` 两项；应用附件封面后网格与详情立即更新；重新打开选择器，选自动后取消仍保留原用户封面。此自动 UI 验证不替代 Mobert 本人的人工验收或 Linux 本地验证，相关项未勾选。

### 审查修复

- 成功 reload 和失联根目录恢复时递增封面 revision；共享缓存观察该 revision 后清除已就绪/失败条目与等待队列。旧任务继续占用并发槽直到结束，以 revision 和条目身份阻止陈旧结果写入新缓存；运行任务以 ID 集合计数，无递减下溢，始终最多四个任务。
- 公共 GPUI 红绿回归覆盖「读取失败 → 文件可用 → 库刷新 → Ready」和「Ready → 同路径内容损坏 → 刷新重新解码 → Unavailable」，不比较像素。
- 附件与封面候选共用 `voon_core::covers::discover_images`；公共测试验证目录范围、忽略、取消及不跟随子符号链接。嵌入封面仍只由选择器枚举。
- 相关定向测试：core `covers` 3/3；app `album_grid` 3/3、`album_detail` 3/3、`cover_picker` 1/1、`rescan` 5/5，通过共 15 项。
- 合并来源交叉修复：Album 与 AlbumRow 保存显式 `source_directories`，独立迁移追加 JSON 列，旧快照可反序列化；合并/拆分由 ticket 16 传播真实来源。图片发现、详情位置与显示目录均读取真实来源，不推断任意 Disc 的父目录。core `covers` 4/4（含合并来源候选）、`attention` 5/5（含物理问题关联）通过。
- 跨根目录合并时，附件与选择器统一汇总所有根目录的绝对忽略路径。GPUI 回归先观察次要根目录 private 图片泄漏（3 张而非 2 张），修复后 app `album_detail` 5/5，且 reveal 指向真实 primary 来源。覆盖 grid/detail/picker/rescan 的 scoped Clippy `-D warnings` 通过。
