---
status: ready-for-agent
---

# 05: 列表视图

**What to build:** 专辑区可以在网格与列表之间切换。列表视图有 8 列（按原型：如标题、专辑艺术家、作曲家、年份、格式、分辨率、Disc/Track 数、所属 Collection），点击列头可升序/降序排序。顶部搜索框按标题、艺术家、作曲家、唱片编号过滤，中英日文均可。

**Blocked by:** 03 专辑网格；外部门槛：原型审核通过（PR #1 / https://voon-design-canvas.vercel.app）

**Status:** ready-for-agent

- [ ] 网格 / 列表切换后保留当前筛选和选中项
- [ ] 列表显示 8 列，列定义与原型一致（人工检查）
- [ ] 点击列头按该列排序，再次点击反向
- [ ] 搜索框按标题、艺术家、作曲家、唱片编号过滤，支持中文、日文、英文
- [ ] 状态层测试：切换视图模式、按列排序、输入搜索词后，状态层暴露的 Album 顺序与集合符合预期
- [ ] 对应规格中的用户故事：55、56、57、59、60
- [ ] Linux 本地验证通过（build / clippy -D warnings / test --workspace）
- [ ] macOS 由 Mobert 手动验证

规格：[0001 第一期规格](../../0001-phase-1.md)
