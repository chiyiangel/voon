---
status: ready-for-agent
---

# 18: macOS 打包

**What to build:** 在第一期末把 Voon 打包成带图标的 macOS .app，可从"应用程序"或程序坞启动；不做代码签名和公证（个人使用），README 说明首次打开时如何放行。Linux 不在本工单范围。

**Blocked by:** 03 专辑网格

**Status:** ready-for-agent

- [ ] 生成带图标的 Voon.app，双击可启动并扫描默认 Library root
- [ ] README 说明打包步骤与首次打开的放行方法
- [ ] 不引入签名或公证步骤
- [ ] 对应规格中的用户故事：103
- [ ] Linux 本地验证通过（build / clippy -D warnings / test --workspace），打包改动不影响 Linux 构建
- [ ] macOS 由 Mobert 手动验证

规格：[0001 第一期规格](../../0001-phase-1.md)
