---
status: ready-for-agent
---

# 06: 专辑详情

**What to build:** 选中 Album 后右侧详情显示：封面、全部专辑级字段及其 Field source（标签 / CUE / 目录名 / 我的编辑）、所属 Collection；曲目按 Disc 分组；Virtual track 有明显标记，并显示对应的 Whole-disc image 与 CUE sheet；每首 Track 显示格式、采样率、位深、时长；附件目录（Booklet 等）中的图片可查看；可以在 Finder / 文件管理器中显示该 Album。voon-core 需要提供按 Album 查询 Disc / Track 明细的接口。

**Blocked by:** 03 专辑网格；外部门槛：原型审核通过（PR #1 / https://voon-design-canvas.vercel.app）

**Status:** ready-for-agent

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
