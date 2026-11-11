---
status: in-progress
---

# 18: macOS 打包

**What to build:** 在第一期末把 Voon 打包成带图标的 macOS .app，可从"应用程序"或程序坞启动；不做代码签名和公证（个人使用），README 说明首次打开时如何放行。Linux 不在本工单范围。

**Blocked by:** 03 专辑网格

**Status:** in-progress

- [ ] 生成带图标的 Voon.app，双击可启动并扫描默认 Library root
- [ ] README 说明打包步骤与首次打开的放行方法
- [ ] 不引入签名或公证步骤
- [ ] 对应规格中的用户故事：103
- [ ] Linux 本地验证通过（build / clippy -D warnings / test --workspace），打包改动不影响 Linux 构建
- [ ] macOS 由 Mobert 手动验证

规格：[0001 第一期规格](../../0001-phase-1.md)

## 实现与验证记录（2026-11-11）

- `scripts/package-macos.sh` 支持 release 与 `--debug`，生成 `dist/Voon.app`、Info.plist 和可复现的暖陶土色音符图标，无签名/公证步骤。
- debug 构建和图标转换已在本机完成，`plutil -lint`、图标尺寸/格式检查通过。脚本语法检查通过。
- README 已记录打包、本地索引位置、首次打开方式和 Apple 官方说明。
- 最终 release 包已在所有切片集成后构建成功，Info.plist 校验通过。debug 包的实际窗口主流程验收通过；最终 release 包生成时 Mac 已锁屏，未再次进行窗口操作。Linux 回归和 Mobert 手工验收仍未完成。
