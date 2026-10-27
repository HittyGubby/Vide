# 播放分析（跳过静音 / 音量均衡）

服务端负责分析，播放端负责应用。

## 分析

- Rust：`features/analysis.rs`，调用系统 `ffmpeg` 一次完成：
  - `silencedetect` 找歌头/歌尾静音边界（-50 dB，至少 0.4s）；
  - `ebur128` 测整首 integrated LUFS。
- 结果缓存到 `<data-dir>/cache/analysis/<song-id>.json`，按媒体文件的 size + mtime 失效。
- 没有 ffmpeg、解码失败或格式不支持时返回空结果，播放端自动跳过效果。

## 暴露方式

- Web：`GET /api/v1/analysis/:id`（`public_media` 时游客可读，否则需要会话）。
  对在线歌曲该请求会先确保完整缓存（可能触发下载），之后 `/media` 直接读缓存。
- 桌面 / 服务端 mpv：播放器内部直接调用分析，不经过前端。

## 应用

- **mpv**：`--start` / `--end` / `--af=volume=<dB>`；`status()` 和 `seek()` 使用裁剪后的时间轴。
- **浏览器**：播放前取分析；`currentTime = start`，到 `end` 停止并切下一首；
  WebAudio `GainNode` 应用增益（跨域媒体退化为 `audio.volume` 衰减）。
- 增益 = `clamp(target_lufs - lufs, -12, +6)` dB。

## 在线流

- 浏览器 `/media` → `ensure_media_path` 会完整下载到 `cache/<id>.media`，与本地文件同样分析。
- 服务端 mpv：
  - `UrlStream` 远端 URL 现在先完整缓存再播放；
  - `Pipe` 在启用效果时等待完整下载后分析，否则保持边下边播（首次分析在下次播放生效）。

## 设置

- 设置 → 播放：
  - 跳过歌头/歌尾静音；
  - 音量均衡开关 + 目标 LUFS（默认 -16，范围 -30..-6）。
- 游客（Web 未登录）存 localStorage（与主题共用 `vide.guest-theme`）；登录/本地写 SQLite。
- 在线流首次播放可能需要等待下载 + 分析；本地文件通常 < 1s。
