---
status: ready-for-agent
---

# 13: 用默认播放器打开

**What to build:** 在 Album 或 Track 上提供"用默认播放器打开"，在 macOS 与 Linux 上都调用系统默认程序打开对应音频文件（Virtual track 打开其整轨文件）。Voon 自身不播放。

**Blocked by:** 06 专辑详情；外部门槛：原型审核通过（PR #1 / https://voon-design-canvas.vercel.app）

**Status:** ready-for-agent

- [ ] Album 与 Track 上都有"用默认播放器打开"
- [ ] 在 Linux 上实际打开系统默认程序；macOS 由 Mobert 验证
- [ ] Virtual track 打开对应的 Whole-disc image
- [ ] 状态层测试：触发打开操作时，状态层向注入的"打开器"传递正确的文件路径（不真正启动程序）
- [ ] 对应规格中的用户故事：98
- [ ] Linux 本地验证通过（build / clippy -D warnings / test --workspace）
- [ ] macOS 由 Mobert 手动验证

规格：[0001 第一期规格](../../0001-phase-1.md)
