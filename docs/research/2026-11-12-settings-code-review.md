---
status: done
date: 2026-11-12
---

# 0003 菜单与设置窗口代码审查

按 code-review 技能，由两个全新上下文的只读 agent 分别执行 Standards 与 Spec 审查。固定点为文档基线 `155b47a90b69311d3b6871cadeb186a4881ceb6f`；审查时 HEAD 为 `3bba6b05a01566f72d856430c85b25b53a8799ec`。审查命令为 `git diff 155b47a90b69311d3b6871cadeb186a4881ceb6f...HEAD`，提交列表为 `3bba6b0 feat: move library settings to application menus and a separate window`。

标准来源为 README、GLOSSARY、ADR 0001–0005 及技能的代码异味基线；备份与恢复 ADR 的实施继续延期。规格来源为 0003 及其六张已确认工单。工作区测试 204 项与严格 Clippy 已通过，但实际平台验收仍单独核对。

## Standards

未发现需要报告的 Standards 问题（0 项）。

变更符合 README、GLOSSARY 及 ADR 0001–0005 中的中文界面、音乐库只读、领域术语与平台边界约定；未发现值得提出的基线代码异味。Linux 实际窗口尚待验收属于验证边界，不据此判定代码违规。

## Spec

未发现有充分源码依据的实现缺陷、明确遗漏或范围扩张。共享状态、目录路径 payload、扫描保护复查、编辑与封面草稿保护、设置错误反馈及入口迁移均符合规格；未加入备份或数据库结构变更。

验收缺口单列，不计作代码 bug：

- macOS 的浏览位置与草稿保留、扫描中关闭设置、故障恢复及索引切换后的菜单更新仍待实际窗口确认。
- Linux X11、Wayland 的构建和窗口行为尚未验证。

规格明确要求：“不可用的平台如实列为未验证，不以 macOS 或状态层结果替代。”当前记录遵循此要求，不能据此宣称全部平台验收完成。

后续实际窗口结果以[0003 验收记录](../specs/0003-settings-menubar/acceptance.md)为准；本审查的零发现不替代该记录中的平台和窗口待办。

Standards 共 0 项，最严重问题无；Spec 共 0 项，最严重代码问题无，仍有独立验收待办。
