# Voon 一期需求文档（PRD v1.0）

## 1. 产品定位
Voon 是一个类 Roon 的 headless 音乐播放服务。运行在 macOS 上，读取 NAS 曲库，通过 USB DAC 做 bit-perfect 输出到功放，手机在局域网内通过网页/API 控制。

## 2. 技术选型
- 语言：Rust
- 音频输出：直接调用 CoreAudio HAL（coreaudio-rs / coreaudio-sys），不使用 cpal
- 解码：Symphonia
- 存储：SQLite（含 FTS5 全文搜索）

## 3. 一期范围（P0）
1. Voon Core：后台常驻，launchd 开机自启，配置文件指定曲库路径和输出设备。
2. 曲库：扫描已挂载的 NAS 目录（SMB/NFS），读取 FLAC、ALAC、WAV、AIFF 的标签与封面，SQLite 索引；支持手动重扫和定时增量扫描（基于 file_size + mtime），不依赖 FSEvents。
3. 播放：CoreAudio hog mode 独占；按曲目原始采样率自动切换，不重采样、不经系统混音；整数物理格式输出；无缝播放。
4. 控制：REST API + WebSocket 状态推送；Bonjour 局域网发现。
5. 手机端：Core 内置网页控制端（可加到手机桌面）：按专辑/艺人/曲目浏览、搜索、播放队列、正在播放、播放/暂停/拖动进度/上一首/下一首。
6. 音量：一期固定满音量输出，由功放控制。

## 4. 关键需求与约束
- 同采样率相邻曲目必须无缝；不同采样率之间允许 <1 秒空白，但不得出现爆音或开头截断。
- 采样率切换须等待属性变化回调确认后再送数据。
- 正常退出、异常退出、收到信号时都必须释放 hog；启动时清理残留占用。
- 实时音频回调内禁止 I/O；独立读取线程预读 ≥30 秒；当前曲目剩余 30 秒时预取下一首。
- NAS 掉线、Mac 休眠唤醒后，扫描和读取能超时、重试并自动恢复。

## 5. DSD 预留
- 一期扫描时索引 .dsf/.dff，tracks 表 codec_kind = DSD、playable = 0；查询接口默认过滤不可播放曲目。
- 二期以 DoP 方式支持 DSD，上线时将 playable 置为 1，无需重新扫描。

## 6. 里程碑与验收
### M1（约 20 人天）：骨架、CoreAudio 输出、播放引擎、曲库扫描、REST/WebSocket API
- 可用 curl 完成全部播放控制。
- 真实 DAC 上连续播放 NAS 曲库 2 小时，无卡顿、无爆音。
- DAC 指示的采样率与文件一致。
### M2（约 15 人天）：网页控制端、Bonjour 发现、稳定性
- 手机网页操作后 1 秒内看到状态更新。
- NAS 掉线、休眠唤醒后自动恢复。

### 通用验收
- 支持 FLAC、ALAC、WAV、AIFF，最高 24bit/192kHz。
- 1 万首曲库首次扫描 ≤10 分钟。

## 7. 一期不做
在线元数据补全与推荐、流媒体（TIDAL/Qobuz）、多区域/分组、RAAT 及网络播放端、AirPlay/Chromecast、DSP/升频/房间校正、多用户、外网远程（ARC）、标签编辑、电台。

## 8. 二期候选
DSD（DoP）、硬件音量（P1）、Linux 版本（ALSA 输出后端）、原生 iOS App、播放历史与收藏。

## 9. 测试方式
CoreAudio 相关实测在 Mobert 的 Mac 上进行；开发提供命令行测试工具及每步预期结果，Mobert 反馈 DAC 显示的采样率和是否有爆音。
