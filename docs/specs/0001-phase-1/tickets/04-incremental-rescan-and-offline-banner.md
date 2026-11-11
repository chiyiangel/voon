---
status: ready-for-agent
---

# 04: 增量重扫与 NAS 离线横幅

**What to build:** 可以手动触发某个 Library root 的重新扫描；重扫只重读大小或修改时间变化了的文件，其余文件的标签来自缓存。NAS 未挂载时不清空索引，照常浏览、搜索、编辑，并在界面顶部显示横幅"音乐库目录不存在或未挂载"；重新挂载后横幅消失，可以重扫。

**Blocked by:** 02 根目录与首次扫描；外部门槛：原型审核通过（PR #1 / https://voon-design-canvas.vercel.app）

**Status:** ready-for-agent

- [ ] 手动重扫入口可用；重扫期间同样显示进度、可取消
- [ ] 未变化的文件不会被重新读取（重扫后报告的磁盘读取数为 0）
- [ ] NAS 未挂载时显示离线横幅，已索引的 Album 仍可浏览和编辑；挂载后横幅消失
- [ ] voon-core 测试（NAS 形状假库）：第二次扫描在文件未变时读取数为 0；修改一个文件后只重读该文件
- [ ] 状态层测试：Library root 不存在时状态为"未挂载"且 Album 列表保持不变；目录恢复后状态回到正常
- [ ] 对应规格中的用户故事：8、9、11、12
- [ ] Linux 本地验证通过（build / clippy -D warnings / test --workspace）
- [ ] macOS 由 Mobert 手动验证

规格：[0001 第一期规格](../../0001-phase-1.md)
