---
status: done
---

# 10: 全库封面预缓存与失败重试

**What to build:** 将持久封面能力扩到所有已入库 Album，不要求逐张浏览；独立显示预缓存进度，区别无图、未缓存、处理中、已就绪和失败，并能重试失败封面。

**Blocked by:** [04 当前 Cover 持久化与离线重开](04-persistent-cover-thumbnails.md)

**Status:** done

- [x] 扫描接受或加载全部 Album 后，当前自动和手选 Cover 都进入后台缓存，不依赖视口请求。
- [x] 缓存任务有界，不为全库常驻全部解码图像，内存淘汰不删除当前封面的持久资产。
- [x] 扫描进度与封面进度分开；存在失败或尚未完成时不声称全部封面已就绪。
- [x] 单项失败显示明确状态，来源恢复后可重试，不必重扫全部音乐。
- [x] 更换封面、移除根或出现过期任务时，状态和最终图片对应最新有效选择。
- [x] 未浏览 Album 及超过内存容量的封面，在缓存就绪后重建 AppState/Index、断开来源仍可显示。
- [x] 用真实本地资产和 GPUI 状态证明全库队列、失败恢复及离线加载。
- [ ] 进程冷启动窗口验证由 11 在统一真实窗口验收中执行；本次未运行真实 Voon 窗口或访问真实 NAS。
- [x] 不扩展为小册子、原图收藏、历史封面管理或 Library backup。

**Demo:** 不逐张浏览便让一批专辑封面缓存完成，包含一张故障图；恢复来源并重试，再冷启动离线查看未浏览过的专辑。

**User Stories:** 4、55–67。

规格：[0002 资料库扫描可靠性与离线浏览](../../0002-library-reliability-and-protection.md)。通用约束与实施前沿见[工单索引](README.md)。

## 实施与验证记录（2026-11-11）

- AppState 在窗口绑定或生命周期 tick 后为全部已载入 Album 排队；接受扫描、手选封面和移除根后的 reload_library 同步新的当前集合。无需网格、列表或详情逐项请求。AppState::new 仍只读本机 Index，后台任务在有 GPUI Context 后启动。
- 新增公开 cover_cache_progress、album_cover_state、prefetch_covers 和 retry_failed_covers；进度以 Album 计数，与扫描状态及像素驻留分离。ThumbnailState 补充无封面和未缓存，网格占位及独立封面横幅分别呈现状态；来源恢复后使用“重试失败封面”，不必重扫。
- 4 项后台并发、最多 256 张解码图驻留；轻量状态保留全部当前封面，本地资产不受 LRU 影响。外置文件和本地资产读取有字节上限，解码分配有上限；浏览缓存观察者只同步已请求的有界集合，不在每项后台完成时复制整个资料库的图片状态。
- TDD：先以不存在的全库进度/状态接口获得 red，真实未浏览自动图和手选图预取并离线重开后 green；失败重试接口 red→green；260 项场景先复现重解码把就绪计数从 260 降至 259，保留本地资产状态与路径后 green。真实 DiskScanner 接受结果、源文件内容/修改时间不变、移除根退休旧任务另有状态层回归。占位 DFF 不代表真实 DFF/SMB 样本已经验收。
- 独立 target 下 `cargo test -p voon-app --test cover_prefetch --test persistent_covers --test album_grid --test cover_picker --test library_scan --test rescan --locked --offline`：4 项预取测试、7 项持久资产测试和 12 项既有浏览/扫描回归通过。`cargo check --workspace --locked --offline`、严格 app Clippy、格式与差异检查通过。
- 未做实际进程退出后的网格/详情窗口验收、真实 NAS/约 20 张样本、Linux/X11/Wayland 验收；这些外部结论由 11 统一记录。本票的 done 指实现与自动状态层验证完成，不替代上述待办。
