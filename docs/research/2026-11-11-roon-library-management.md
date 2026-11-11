---
status: draft
date: 2026-11-11
source_commit: 2f847c3b8208db9bffe8cdab7ed109529e00d6e8
---

# Roon 音乐资料库管理分析与 Voon 改进报告

Voon 下一阶段最值得投入的是扫描可信度、整理成果保护、可解释的元数据，以及更有效的检索。Roon 的参考价值集中在分层元数据、版本关联、作品与演奏关系、可保存的组合筛选和数据库备份；这些能力需要结合 Voon 的只读 NAS、CUE 整轨和离线使用目标取舍。

本报告包含已核实的实现事实、三项临时假库复现，以及功能方案。**本轮已整理为 ready-for-agent 规格：从新库重新扫描，聚焦 F1–F5 及通用资料库版本检查；备份恢复延期，旧库迁移不在范围内。** 发行边界记录于 ADR 0005，备份相关 ADR 0006、0007 已标记实施延期。具体实现与验收以 [0002 资料库扫描可靠性与离线浏览](/Users/liujun/Private/Code/github/voon/docs/specs/0002-library-reliability-and-protection.md) 为准，研究报告中的后续方向不自动成为本轮开发要求。

## 分析对象与证据范围

| 对象 | 本次基线 |
| --- | --- |
| Roon 安装包 | `/Users/liujun/Downloads/Roon.dmg`，SHA-256 `84f0fcd86d648f13f6767cb9742269763bd38699881c30141a248cb1ee2b3297` |
| Roon 产品版本 | `Contents/Resources/VERSION`：**2.74 build 1719 production**。Info.plist 和程序集中的 `0.0.1` 是包装版本，不能用作产品版本。 |
| Voon | 提交 `2f847c3b8208db9bffe8cdab7ed109529e00d6e8`，Rust 核心、GPUI 状态层和实际 UI 回调均已核对。 |
| Roon 分析方式 | REA 静态托管程序集分析、CIL 调用关系，辅以只读提取的版本文件和 BROO 界面资源，以及 Roon 官方文档。未启动 Roon、登录账户或导入音乐库。 |
| Voon 验证方式 | 现有离线测试 123 项通过；另用临时程序验证子目录不可读、标签负缓存和 DFF 加 CUE 时长。没有扫描真实 NAS，也没有操作用户数据库。 |

REA 的 DMG 整盘枚举被 `.Trashes` 权限限制阻断，随后通过自建只读挂载直接选取程序集分析。该挂载已卸载。`Roon.Broker.Api.dll` 与 `Roon.Storage.Directory.dll` 的成员检查完整；`Roon.Metadata.dll` 的部分字符串带有格式限制；`Roon.Broker.Core.dll` 的成员解码超过 64 MiB 表示预算。因此，识别评分、完整合并算法和身份迁移策略没有被恢复，不能由接口名推断其实现正确性。

## Roon 的资料库能力与可借鉴之处

### 元数据有来源层和选择策略

REA 在 `EditRequiredRefInfo<T>` 中观察到 `LocalValue`、`MetadataValue`、`EditValue`、`PreferLocalValue` 及各层存在标志；列表字段另有增加和移除值。`LibrarySettingsInfo` 包含专辑、碟片、曲目及不同参与角色的来源偏好。这说明它的编辑接口能分别表达文件值、在线资料值和人工值，而不只是显示一个来源标签。证据见下文 E1。

官方说明也允许全局或逐项偏好文件资料，并对作曲家、指挥、乐团等角色分开回退。Voon 可以借鉴“保留候选与选择依据”，但在线服务的默认优先级应另行决定。[Roon Import Settings](https://help.roonlabs.com/portal/en/kb/articles/import-settings)

### 版本关联与专辑结构纠错分开

安装包的 API 包含 `AlbumEditInfo.Version`、`DuplicateOf`、`GetAlternateAlbumVersions`，界面资源包含 `Group alternate versions`。官方的 Versions 功能支持比较版本、指定主版本，以及加入或移出版本组。版本仍分别保留曲目和格式等信息。[Roon 版本管理说明](https://help.roonlabs.com/portal/en/kb/articles/faq-how-does-roon-handle-multiple-versions-of-the-same-album)

对 Voon 的直接启发：**把两张误拆的碟合成一张专辑，与把首版和重制版关联起来，是两种操作。** 现有“合并”应继续用于前者。

### 古典乐包含作品和演奏关系

REA 观察到 `WorkLite` 的作曲家、作品目录号、演奏数量，以及 `PerformanceLite` 的作品、时间、地点和参与者；`TrackLite` 则分别暴露录音标识、演奏、段落和乐章名。这些接口支持将作品、一次演奏和承载它的曲目分别表达，不能将它们都压成专辑上的“作曲家”字符串。证据见 E3。

官方文档说明 `WORK`、`PART`、`SECTION` 可控制多段作品，`WORKID` 可区别同一作品的不同演奏；人员资料可以表达角色。这些规则有助于设计 Voon 的导入，但不意味着可以直接用 Roon 的标签名充当 Voon 的实体 ID。[Roon 文件标签说明](https://help.roonlabs.com/portal/en/kb/articles/file-tag-best-practice)

### 筛选既能组合也能保存

REA 在专辑查询中观察到格式、存储位置、作曲家、指挥、封面缺失、未识别等条件，以及对应的包含与排除条件。`SmartTagCriteria` 还分别承载不同对象的查询条件；界面资源包含书签保存与管理入口。官方 Focus 支持组合条件、排除条件，并将视图保存为书签。[Roon Focus 与书签](https://help.roonlabs.com/portal/en/kb/articles/faq-how-do-i-create-a-smart-playlist-how-do-i-save-a-focus)

Voon 可以先支持“保存当前筛选”，随后再决定是否扩展为包含排序、视图和更多对象的完整书签。

### 扫描维护区分失败、不可用和主动清理

在 `Roon.Storage.Directory.dll` 中，REA 观察到实时监视、强制扫描、失败记录和重试方法。CIL 调用明确包含 `_ScanDirectory → _LogScanFailure / _QueueScanRetry` 和 `_RetryScanDirectory → _ScanTree`，这比单独出现“重试”字符串更强，但仍不是 NAS 运行验证。见 E5。

维护 API 分别统计已删除文件、已移除存储配置和已停用存储中的曲目。官方也把 Clean Up Library 与重新扫描区分开：前者主动忘记原有记录，后者检查存储变化。Voon 应明确“暂时读不到”和“确认从索引移除”的边界。[Roon Clean Up Library](https://help.roonlabs.com/portal/en/kb/articles/clean-up-library)

### 整理成果可以独立备份

REA 观察到自动备份、手动备份、恢复、进度、最近成功时间和错误状态。官方明确数据库备份覆盖编辑、合并、标签等个性化成果，不包含音乐文件。[Roon 数据库备份说明](https://help.roonlabs.com/portal/en/kb/articles/what-is-a-backup-in-roon)

这点与 Voon 尤其相关：音乐文件只读，并不能保护仅存在本机数据库中的整理劳动。

### 套装允许碟片保留自己的资料

REA 的 `DiscEditInfo` 包含独立标题、年份、封面和关联发行。官方文档也说明碟片级编辑及其来源偏好。对于 Philips Duo 等收藏，Voon 后续可以保留每碟的发行背景，而不只是显示“CD1、CD2”。[Roon 多碟套装说明](https://help.roonlabs.com/portal/en/kb/articles/box-sets-how-to-check-your-files)

## Voon 已有能力

现有功能已经有真实核心实现、GPUI 状态操作和界面入口，不能重新列为“尚未实现”。

| 能力 | 当前实现与边界 |
| --- | --- |
| 多根目录及目录结构 | 自动区分 Album、Disc、嵌套 Collection；根目录散文件按标签归组。见 [scan.rs:332](/Users/liujun/Private/Code/github/voon/crates/voon-core/src/scan.rs:332)、[scan.rs:669](/Users/liujun/Private/Code/github/voon/crates/voon-core/src/scan.rs:669)。 |
| 整轨加 CUE | 生成 Virtual track，保留映像、CUE 和时间范围，不切文件；支持多种中日文编码。见 [scan.rs:500](/Users/liujun/Private/Code/github/voon/crates/voon-core/src/scan.rs:500)、[cue.rs:51](/Users/liujun/Private/Code/github/voon/crates/voon-core/src/cue.rs:51)。 |
| 本地索引及增量扫描 | 大小加纳秒 mtime 标签缓存；后台扫描、取消、根目录离线保护、空挂载点确认。见 [db.rs:333](/Users/liujun/Private/Code/github/voon/crates/voon-core/src/db.rs:333)、[scanning.rs:101](/Users/liujun/Private/Code/github/voon/crates/voon-app/src/view_model/scanning.rs:101)。 |
| 浏览与筛选 | 网格、列表、目录套装、作曲家、分面、标签、手动合集、待处理；同维度 OR，跨维度 AND。见 [sidebar.rs:75](/Users/liujun/Private/Code/github/voon/crates/voon-app/src/ui/sidebar.rs:75)、[facets.rs:122](/Users/liujun/Private/Code/github/voon/crates/voon-app/src/view_model/facets.rs:122)。 |
| 编辑与结构纠错 | 专辑字段编辑及恢复、封面选择、标签、手动合集、合并、按碟或选中曲目拆分、撤销。结构操作会在重扫后重放。见 [edits.rs:35](/Users/liujun/Private/Code/github/voon/crates/voon-core/src/db/edits.rs:35)、[structure.rs:173](/Users/liujun/Private/Code/github/voon/crates/voon-core/src/db/structure.rs:173)、[merge_sheet.rs:33](/Users/liujun/Private/Code/github/voon/crates/voon-app/src/ui/merge_sheet.rs:33)。 |
| 改名候选 | 同一根目录内，消失与新增路径有唯一内容签名匹配时，提议迁移用户整理数据，需人工确认。见 [db.rs:693](/Users/liujun/Private/Code/github/voon/crates/voon-core/src/db.rs:693)。 |

Roon 在这份安装包中的 CUE、APE、WavPack 端到端兼容性没有验证。本次不能据其界面与 API 反推格式兼容性；Voon 已有的 CUE 和目录解释能力应作为自身基线保留。

## 优先修正的现有缺口

### F1 局部扫描失败会被当作完整快照

**假库实测。** A、B 两张专辑已入库；A 目录改成不可读，B 正常。扫描报告 A 为 `Unreadable`，但 `cancelled=false`，根目录可读且设备未变。按当前应用的完成分支调用 `store_scan` 后，索引由两张变成仅剩 B。

根因链条是：子目录错误返回空结果；只有根目录自身不可读才标记整体不可用；非取消且仍有专辑的扫描走完整快照替换。这使“未成功观察到”变成了“确认不存在”。音乐文件没有被删除，也不能将此现象表述为用户编辑已永久丢失。见 [scan.rs:337](/Users/liujun/Private/Code/github/voon/crates/voon-core/src/scan.rs:337)、[scanning.rs:104](/Users/liujun/Private/Code/github/voon/crates/voon-app/src/view_model/scanning.rs:104)、[scanning.rs:294](/Users/liujun/Private/Code/github/voon/crates/voon-app/src/view_model/scanning.rs:294)、[db.rs:347](/Users/liujun/Private/Code/github/voon/crates/voon-core/src/db.rs:347)。

**建议：** 记录扫描成功覆盖的子树和失败子树，只在成功完整观察的范围确认移除；失败范围保留上次快照并显示陈旧状态。不能把所有错误都当成取消，否则真正删除的专辑永远无法收敛。

**验收：** A 暂时不可读时仍能离线查看其旧资料；B 的新增和删除照常更新；A 恢复后重试成功，错误状态解除。

### F2 标签读取失败会形成不重试的负缓存

**接缝故障注入实测。** 第一次读取返回 `None`；随后换成能成功读取的 Reader，保持文件 stamp 不变，再次扫描底层读取次数为 **0**，资料仍缺失。无缓存对照可以成功读出标签。这验证了缓存行为，没有模拟真实 SMB 故障。

`DiskTags` 把错误经 `.ok()` 转成 `None`，缓存又将该结果与相同 stamp 绑定，无法区分无标签、不支持格式与暂时 I/O 失败。见 [scan.rs:122](/Users/liujun/Private/Code/github/voon/crates/voon-core/src/scan.rs:122)、[db.rs:618](/Users/liujun/Private/Code/github/voon/crates/voon-core/src/db.rs:618)、[db.rs:776](/Users/liujun/Private/Code/github/voon/crates/voon-core/src/db.rs:776)。

**建议：** 分开表示成功但无标签、不支持格式、临时失败、解析失败；失败保留上次成功资料并可重试，提供绕过标签缓存的强制重读。强制重读与普通增量扫描的文案要区别清楚。

**验收：** 文件不变、读取恢复后可以补齐资料；不支持的格式不会无限重试；短暂失败不抹去已知字段。

### F3 字段候选没有按规格完整保留

**确定的规格差距。** 一期 Q2 要求保存所有候选值及来源，但 `resolve_metadata` 已先按优先级合并；标签内部再按多数值选出一个值。入库时只有归并后的字段。数据库允许不同来源行，并不代表扫描已保存它们。见 [规格:199](/Users/liujun/Private/Code/github/voon/docs/specs/0001-phase-1.md:199)、[scan.rs:840](/Users/liujun/Private/Code/github/voon/crates/voon-core/src/scan.rs:840)、[scan.rs:857](/Users/liujun/Private/Code/github/voon/crates/voon-core/src/scan.rs:857)、[details.rs:188](/Users/liujun/Private/Code/github/voon/crates/voon-core/src/db/details.rs:188)。

例子：文件标签写 1999，CUE 写 1988，目录也带年份。现在能看到胜出的值及来源，无法比较被舍弃的值。将来接入在线查询后，这种信息损失会更明显。

补充核对：旧 `file_cache` 可能仍有部分逐文件标签值，目录名也可以重解析。因此不能说所有落选资料都已消失；但完整的历史候选、CUE 证据和采纳意图无法靠数据库升级可靠补齐。用户在 Q10 决定从新库重新扫描，本轮不实现旧库来源补齐。

**本轮方案：** 保存候选值、来自哪个文件或 CUE、观测时间及默认选择依据；自动值与用户覆盖分层，详情可以比较候选。保留年份原值和出处，方便用户判断含义；年份语义拆分、来源偏好和完整冲突审阅留到后续，不新增本轮字段或批量工作流。

**验收：** 所有来源可比较；恢复自动值不丢候选；重扫保留人工选择；同一专辑内不同曲目的人员或作品资料不会被多数票吞掉。

### F4 CUE 信息没有完整走到时长和界面

**时长已实测，演奏者展示由源码确认。** DFF 加两轨 CUE，首轨从 0 到 13500 帧，应能算出 180000 ms；`NoTags` 和 `DiskTags` 两路扫描、入库往返后仍是 `duration_ms=None`。原因是没有 TagInfo 时在计算 CUE 时长之前就 `continue`。见 [scan.rs:630](/Users/liujun/Private/Code/github/voon/crates/voon-core/src/scan.rs:630)。

两轨演奏者均正确保存在核心模型中，但详情 `track_row` 没有渲染 `track.artist`。见 [album_detail.rs:409](/Users/liujun/Private/Code/github/voon/crates/voon-app/src/ui/album_detail.rs:409)。

**建议与验收：** 有明确 CUE 终点的轨道独立计算时长，末轨缺整轨总长时保留未知；详情显示曲目级演奏者。默认播放器打开 Virtual track 当前实际打开整个映像，文案应明确这一边界；是否增加定位跳转属于后续能力选择。见 [player.rs:18](/Users/liujun/Private/Code/github/voon/crates/voon-app/src/view_model/player.rs:18)。

### F5 离线浏览缺少持久封面

**源码确认，尚未单独进行断网窗口验收。** `CoverCache` 是上限 256 项的内存缓存，重新加载封面仍从音乐目录或音频读字节。关闭程序、断开 NAS、再次启动，文字索引存在，但没有本地缩略图保证。见 [grid.rs:75](/Users/liujun/Private/Code/github/voon/crates/voon-app/src/view_model/grid.rs:75)、[grid.rs:117](/Users/liujun/Private/Code/github/voon/crates/voon-app/src/view_model/grid.rs:117)、[grid.rs:157](/Users/liujun/Private/Code/github/voon/crates/voon-app/src/view_model/grid.rs:157)。

**建议：** 将缩略图持久化到本机，以来源变化和封面选择修订使缓存失效。验收必须包含退出、断开 NAS、重新启动；仅在同一进程断开网络不足以验证持久性。

## 适合增加的能力

### A1 整理成果备份与可恢复导出（延期）

用户已决定本轮不实现备份与恢复。以下保留为未来设计，不作为 M0 的依赖或验收项。

当前源码没有资料库备份或导出入口。字段编辑、封面选择、标签、合集和结构纠错都在本机，重扫只能重建原始观察，不能重建全部人工劳动。

已讨论的完整本地状态备份范围和恢复保护见 ADR 0006、0007，均已标记实施延期。Q11 的自动备份时机没有被接受，当前不再追问。

留存的设计讨论包括：普通自动备份保留最近 20 份，以及最近 7 个有备份日期每天最后一份，合并去重最多 27 份；手动和保护份不参与普通轮换，新备份验证通过后才清理旧份。这些约定随功能延期，重新启动开发时再核对。曾提出的每 30 分钟自动备份建议未被接受。

未来实现时，备份设计覆盖用户编辑、结构操作、组织关系、根目录映射、封面选择和版本信息。曲目清单导出和可恢复备份使用不同名称，并通过实际空库恢复验证；具体方案在开发重新启动时再核对。

### A2 专辑身份与存放位置分开

当前 Album 和用户数据主要以路径定位。Content signature 是相对音频路径与大小的签名，不是音频内容指纹；跨根目录移动、重编码或挂载路径变化不在现有改名候选的承诺内。这是 [ADR 0004](/Users/liujun/Private/Code/github/voon/docs/adr/0004-rename-candidates.md) 的明确取舍，不能直接当作一期 bug。见 [model.rs:293](/Users/liujun/Private/Code/github/voon/crates/voon-core/src/model.rs:293)、[model.rs:318](/Users/liujun/Private/Code/github/voon/crates/voon-core/src/model.rs:318)。

候选方案是为整理对象保留稳定身份，把路径作为可变的位置；签名用来推荐重新关联，而不充当唯一身份。用户已确认同一份文件在不同位置的存放实例为 Copy；本机与 NAS 的副本是否共享编辑、两个独立收藏是否允许故意保留不同整理，以及关联错误如何撤销，仍待讨论。Roon 暴露 PersistentAlbumId 只能证明存在持久标识接口，不能证明它采用某一种指纹或迁移算法。

### A3 相关版本与副本比较

现有 `Album` 在词汇表中表示一份具体发行；`edition` 和 `catalog_number` 是字段，没有版本组或副本关系。当前合并会汇总 Disc，不能用来代替版本关联。见 [GLOSSARY.md:16](/Users/liujun/Private/Code/github/voon/GLOSSARY.md:16)、[model.rs:63](/Users/liujun/Private/Code/github/voon/crates/voon-core/src/model.rs:63)。

用户已确认“CD 首版、2015 重制版”保持为独立 Album，通过 Version group 关联；同一文件集合的本机副本属于 Copy。副本是否共享同一整理对象仍待确认。候选比较视图展示发行号、年份含义、曲目差异、技术属性和位置；默认展示偏好不能隐含地合并内容，也不能根据采样率断定哪个版本更好。见 [ADR 0005](/Users/liujun/Private/Code/github/voon/docs/adr/0005-albums-version-groups-and-copies.md)。

### A4 多人物角色与作品浏览

当前作曲家、演奏者、指挥、乐团是专辑级字符串；标签作曲家按整张多数值取一个，演奏者拼接，作曲家浏览按整个字符串分组。见 [scan.rs:933](/Users/liujun/Private/Code/github/voon/crates/voon-core/src/scan.rs:933)、[composers.rs:23](/Users/liujun/Private/Code/github/voon/crates/voon-app/src/view_model/composers.rs:23)。

建议先解决“多人、多角色、别名、作用于哪首曲目”，再推进作品关系。这样一张巴赫与莫扎特合辑能出现在两位作曲家下，`巴赫` 与 `J. S. Bach` 可以经确认关联，同时保留原始标签。乐团资料也可评估从已有文件的 ENSEMBLE 等字段导入，不必全部依赖联网。

作品模型应能表达一部作品的多个乐章、同一作品的不同演奏、某次演奏在不同发行中的出现。**具体实体边界尚待讨论，暂不新增正式术语或迁移数据库。**

### A5 保存筛选和更完整的检索

现有搜索仅覆盖专辑标题、专辑艺术家、作曲家、唱片编号的小写子串；不搜曲目、指挥、乐团、演奏者、版本和用户标签。筛选状态没有跨启动保存。见 [browse.rs:142](/Users/liujun/Private/Code/github/voon/crates/voon-app/src/view_model/browse.rs:142)、[view_model.rs:138](/Users/liujun/Private/Code/github/voon/crates/voon-app/src/view_model.rs:138)。

可以先支持三个日常任务：“卡拉扬指挥的贝多芬”“缺少封面的 DSD”“我标记为待核对的台湾首版”。保存条件后，新入库的匹配专辑应自动出现；这与有固定有序成员的 Manual collection 不同。先复用现有分面语义，增加排除、缺失字段和保存功能，再按实际需求考虑更复杂的规则编辑器。

### A6 从待处理进入可完成的审阅流程

来源冲突、重新关联、缺封面和缺角色需要不同处理动作，不应都停留在一条错误说明。建议从 F3 的候选比较开始，支持跳转、采纳、保留现值和撤销；批量处理先展示影响范围与前后差异。多选编辑还需区别“保持原值”“设为空”“统一设值”，避免将界面空白误当成用户要清空字段。

在线查询宜在候选保存后加入。用户已确认默认按需查询、比较后采纳，并保护已有人工编辑。查询结果保留外部标识、匹配依据与采纳记录仍是待细化的方案；自动补空值、批量采纳和具体服务选择尚未确定。

## 建议的交付顺序与验收

| 里程碑 | 范围 | 用户能验证的结果 |
| --- | --- | --- |
| M0 可靠性修正 | F1–F5，及通用资料库版本检查 | 部分 NAS 失败不让专辑消失；失败标签能重试；CUE 边界正确；重启后离线有封面；更高版本库不被误写。 |
| M1 整理效率 | A5 保存筛选及扩大搜索，A6 基础审阅 | 每天重复的查找可保存；冲突可比较并处理；新入库专辑自动符合已有规则。 |
| M2 关系模型 | 讨论确定后的 A2–A4；再扩展碟片资料和按需在线查询 | 移动、副本、不同发行和不同演奏各有明确含义；迁移有预览、可验证、可撤销。 |

先 M0 修正、再扩展新能力的顺序继续有效。M0 已整理为状态 ready-for-agent 的 0002 规格；备份恢复及相关验收延期，不阻塞本轮。发布规格不表示功能已经实现，也没有以未经测量的工期作为承诺。播放引擎、流媒体聚合、推荐系统、跨设备远控和文件写回仍不在本轮范围。

本轮真实样本覆盖多编码 CUE、多碟套装、不同标签来源、DSD、子树断连、标签读取恢复及封面更换，并验证重启离线使用。同专辑不同发行、副本和跨根关联在关系模型阶段验证，备份恢复验收延期。当前 123 项测试不能代替这些真实文件和窗口场景。既有平台验收限制见 [acceptance.md:41](/Users/liujun/Private/Code/github/voon/docs/specs/0001-phase-1/acceptance.md:41)。

## 交付决策与后续方向

Q10 采用新库重扫，备份相关分支已延期。D1 中本轮需要的范围、交互与测试约束已经汇入 0002 规格，不再保留备份频率或旧库补齐作为当前问题；D2、D3 下的关系模型和在线查询细节留待之后展开。

| 决策 | 推荐起点 | 后续才需要回答的分支 | 当前状态 |
| --- | --- | --- | --- |
| D1 下一里程碑的顺序 | 先 M0 可靠性修正，再增加日常整理能力 | 本轮按 0002 实现与验收；备份分支延期 | 0002 已发布为 ready-for-agent |
| D2 发行、版本关系和副本的区别 | 延续 Album 表示具体发行；首版与重制独立并可关联；副本另行表达 | 稳定身份、编辑共享范围、主版本、重复候选、迁移与撤销 | 第一轮 Q2 已接受，ADR 0005 |
| D3 在线资料的采纳权 | 按需查询，比较后采纳；已有人工值受保护 | 匹配证据、仅补空值是否可自动、按字段偏好、失败与过期处理 | 第一轮 Q3 已接受 |

`Version group`、`Copy` 和 `Library backup` 保留为已讨论的领域术语，词汇表不表示功能已实现；`Work`、`Recording` 仍是候选术语。ADR 0006、0007 明确实施延期；本轮以正式 0002 规格为开发入口，后续功能不借此获得实现授权。

## 验证记录与复核入口

- `cargo test --workspace --locked --offline`：**123 项通过，0 失败、0 跳过**。本轮没有改动应用源码或增加仓库测试。
- 临时诊断程序三项运行成功：局部不可读导致索引由 2 张变 1 张；同 stamp 的负缓存阻止恢复读取；DFF 加 CUE 明确终点仍缺时长。
- 前两项分别属于核心扫描加应用完成路径复写、TagReader 接缝故障注入；没有人工 GUI 或真实 SMB 故障复现。第三项使用占位 DFF 与真实 CUE；不能据此声称实际 DFF 音频解析已经受测。
- 封面离线重启和曲目演奏者展示的判断来自源码，待补实际窗口验收。符号链接环路也有静态风险，本轮未复现，不列为已经确认的故障。

诊断程序与原始输出位于 [临时诊断目录](/private/tmp/voon-roon-analysis/repro/RUN_COMMAND.txt)。报告配套的 [证据摘要](/Users/liujun/Private/Code/github/voon/docs/research/2026-11-11-roon-library-evidence.json) 保存所选成员坐标、来源哈希、资源观测和诊断材料，便于临时目录清理后仍能复核。

### REA 证据索引

| 编号 | 摘要中的区域 | 精确来源 |
| --- | --- | --- |
| E1 元数据层 | `source_layers` | API 成员父证据 `ev_4c47c8924070d7058f5039f9013bbf7e57f1e2265e88730a8e77cc29516b8a47`；`EditRequiredRefInfo` 的选页证据 `ev_a7656498aac6b5bf0b36ac49c3ed121a7b25445f6599d6684008d5b3aca839dd`。 |
| E2 版本 | `versions` | `AlbumEditInfo` 选页 `ev_daeebca7932978fa23faf74a657692413ca478dfd466265063de5cb1c17e2a5a`；Library 选页 `ev_a190c85b1ceef54e488e1c7c5bf48e6ef5446e25adeb1c6b8e18b3cb51dfa6eb`。 |
| E3 古典关系 | `classical` | WorkLite 选页 `ev_1d751888388038baf1910f72f067cbbfc82fda757f4f6b5a3deb7f7547703d2d`；PerformanceLite 选页 `ev_0948fc20ac8609a77d8b27133f712a6d698165c9364a67cc037c09deb27571f7`。 |
| E4 查询 | `filters` | AlbumQueryCriteria 选页 `ev_5e0c63ed4586961194432017fc835f0145014fb6ad71945a2bf9dff0229e7e63`。 |
| E5 存储扫描 | `storage`、`storage_calls` | 目录成员父证据 `ev_8c7deb2d14d741d563d05d55546cedb89b170cd23461020b1f45fb5c4d3791c8`；CIL 调用选页 `ev_b2b4838cefe1f5b906a4e0e475563bf8dd99f0defc806c7dcd8b9edce25730ae`。 |
| E6 清理 | `maintenance` | LibraryMaintenance 选页 `ev_7830c7151a70af0c90bd1d609854723157435c50af7c0d41d97971d5714a5454`。 |
| E7 备份 | `backups` | Backups 选页 `ev_42664fe85c2fbd3427cf3a16d4bc07ef45f564c4cf58ccdd135d027cf0bdf50a`。 |
| E8 持久标识 | `identity` | PersistentAlbumId 选页 `ev_13d6b622598e6808ee650aff1072c7e082fab65cd1499687c4a7ba25aee74dd9`。 |
| E9 碟片资料 | `box_sets` | DiscEditInfo 选页 `ev_2f9e8895d14e6a1d7a3ab7e785d3d6623b0e78b21e6d1642447a90d0e3bfb02d`。 |

所有元数据 token 必须同时绑定程序集 SHA-256 与 MVID；摘要逐项保存这两个值。完整规范化 REA 导出在 [rea-evidence.json](/private/tmp/voon-roon-analysis/rea-evidence.json)，包含 41 条证据，约 35 MB；它在临时目录中，摘要是便于阅读的派生投影，不能冒充完整规范证据。界面资源观察另有原始压缩文件及解压字节的哈希和匹配偏移；字面量存在不等于界面可达或功能运行成功。
