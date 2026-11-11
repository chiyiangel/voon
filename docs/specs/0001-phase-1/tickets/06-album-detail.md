---
status: in-progress
---

# 06: 专辑详情

**What to build:** 选中 Album 后右侧详情显示：封面、全部专辑级字段及其 Field source（标签 / CUE / 目录名 / 我的编辑）、所属 Collection；曲目按 Disc 分组；Virtual track 有明显标记，并显示对应的 Whole-disc image 与 CUE sheet；每首 Track 显示格式、采样率、位深、时长；附件目录（Booklet 等）中的图片可查看；可以在 Finder / 文件管理器中显示该 Album。voon-core 需要提供按 Album 查询 Disc / Track 明细的接口。

**Blocked by:** 03 专辑网格；外部门槛：原型审核通过（PR #1 / https://voon-design-canvas.vercel.app）

**Status:** in-progress

- [ ] 详情显示封面、专辑级字段与每个字段的 Field source 标记
- [ ] 多碟 Album 按 Disc 分组；Virtual track 与普通 Track 可区分，并能看到整轨文件和 CUE
- [ ] 每首 Track 显示格式、采样率、位深、时长；GBK / Shift-JIS CUE 的曲名显示正确
- [ ] "在 Finder / 文件管理器中显示"在 macOS 和 Linux 上都可用
- [ ] voon-core 测试（NAS 形状假库）：查询 Philips Duo 风格的 Album 得到 2 张 Disc；整轨加 GBK CUE 的 Album 得到带正确曲名与起止位置的 Virtual track；字段带正确来源
- [ ] 状态层测试：选中 Album 后详情状态包含其 Disc 与 Track；切换选中项后详情更新
- [ ] 对应规格中的用户故事：26、27、38、76、77、78、79、80
- [ ] Linux 本地验证通过（build / clippy -D warnings / test --workspace）
- [ ] macOS 由 Mobert 手动验证

规格：[0001 第一期规格](../../0001-phase-1.md)

## 实现与验证记录（2026-11-11）

- `Index::album(path)` 从本地索引返回完整 Album（用户编辑叠加、Disc、Track 与 Virtual track、音频属性与来源），不访问音乐库。
- 右侧详情共享网格封面缓存，显示全部专辑级字段及其来源、Collection、分碟曲目、虚拟区间及 image/CUE 路径、音频属性和时长。首次扫描未完成时也可查看已发现专辑。
- 附件图片列表在后台发现，包含 Album/Disc 和 Booklet 等附件目录，遵守内置与用户忽略规则；点击图片通过系统图片查看器打开。文件管理器显示操作也在后台执行，平台打开器可注入。
- `cargo test -p voon-core --test album_detail`：2 项通过，覆盖 Philips Duo 两碟、GBK/Shift-JIS 曲名与来源、虚拟区间、音频属性及用户编辑。`cargo test -p voon-app --test album_detail`：3 项通过，覆盖选择切换、附件发现和注入系统操作、扫描中的详情。新增索引/状态接口先观察编译失败；扫描中查看先观察行为失败，再实现。
- macOS `cargo check -p voon-app` 与 `cargo clippy -p voon-app --all-targets -- -D warnings` 通过。系统打开器仅通过注入验证了动作与路径；Linux 实际运行、原生界面与 Finder / 文件管理器人工验收仍待完成，暂不标记 `done`。
