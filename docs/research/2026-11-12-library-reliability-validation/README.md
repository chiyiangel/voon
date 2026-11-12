# 0002 自动验证归档

代码快照为 `f2ac75b2ce33da29fd0ad9bac92d3b75e203522c`。2026-11-12 在 macOS 上重新运行，防止已不可用的临时日志成为唯一依据。

- [summary.json](summary.json)：命令、环境、194 项测试统计、日志 SHA-256，以及此前会话观察和外部待验收项。
- [workspace-tests.log](workspace-tests.log)：完整工作区测试输出，0 失败、0 忽略。
- [clippy.log](clippy.log)：全目标严格 Clippy 输出。
- [build.log](build.log)：macOS debug 构建输出。

格式与差异检查也已通过。真实 NAS 和实际窗口的部分结果只保留会话观察，未归档的原始清单不视作仍可复核。见[联验记录](../../specs/0002-library-reliability-and-protection/acceptance.md)。
