# Vide 插件协议 v1

状态：协议已确定。协议类型/one-shot runtime、argv 覆盖、完整路由持久化、启用/禁用、插件管理/路由 UI、插件热重载、M3U 移除、平台路径 resolver、流式缓存和并发上限均已落地。待启动队列/令牌桶细化可作为后续优化。本文是实现依据；旧文档中“插件常驻进程 + stdin 循环”的描述以此文为准。

## 1. 定位

- 插件是外部子进程，核心不捆绑 Node/Python 或其他运行时。
- 插件只负责平台适配，返回结构化数据。
- 核心负责网络、缓存、文件、播放、数据库和路由。
- 插件不接触本地文件路径、缓存和 SongId。
- 插件安全、依赖和日志由插件作者/用户负责。

## 2. 进程模型：一次任务一个进程

插件不常驻。每次能力调用都是一次独立进程：

1. 核心解析路由，得到有效 argv。
2. 以插件目录为工作目录启动进程。
3. 写入一行请求 JSON。
4. 关闭 stdin。
5. 读取一行响应 JSON。
6. 插件退出；核心在短暂宽限期后仍存活则 kill。

- 一个进程只处理一个请求。
- 不保留长期 stdin/stdout 循环。
- 不存在空闲 running 状态，也不存在中途切换 argv。
- 插件可以写成“读一行、输出一行、退出”。

## 3. ID 约定

| 名称 | 含义 |
|---|---|
| `platform` | 平台命名空间，由插件声明 |
| `source_id` | 平台内条目 ID |
| `song_id` | Vide 本地唯一 ID，由核心生成，插件不可见 |
| `request.id` | 请求/响应关联 ID，与歌曲无关 |

平台 ID 由插件在 manifest 中自行声明。核心不维护平台白名单，也不自动合并别名；不同插件写法不一致时，由用户修改 manifest。

## 4. Manifest

```json
{
  "protocol": 1,
  "name": "ytdlp-adapter",
  "version": "1.0.0",
  "description": "yt-dlp provider",
  "exec": ["python3", "adapter.py"],
  "capabilities": [
    { "type": "search", "platform": "youtube" },
    { "type": "resolve", "platform": "youtube" },
    { "type": "stream", "platform": "youtube" },
    { "type": "playlist", "platform": "youtube" },
    { "type": "lyrics", "platform": "youtube" }
  ]
}
```

- `protocol`：协议版本，v1 为 1。
- `name`：插件唯一名称，用于路由和状态展示。
- `version`：插件版本。
- `description`：管理页展示，可选。
- `exec`：默认 argv；用户可覆盖。
- `capabilities`：声明的 type + platform。

## 5. 能力与参数

v1 只有五种能力：

| 能力 | 参数 | 结果 |
|---|---|---|
| `search` | `query`, `platform`, `limit?` | Track[] |
| `resolve` | `platform`, `source_id` | Track |
| `stream` | `platform`, `source_id` | StreamDescriptor |
| `playlist` | `url`, `platform?` | Track[] |
| `lyrics` | `platform`, `source_id` | Lyrics 或 null |

这些参数由协议固定；用户不填写能力参数。用户配置的是插件进程 argv，插件根据 action/platform 自己决定如何调用工具。

## 6. 请求与响应

请求：

```json
{"id":"t123","action":"search","params":{"query":"hello","platform":"youtube"}}
```

成功：

```json
{"id":"t123","result":[]}
```

失败：

```json
{"id":"t123","error":{"code":"timeout","message":"..."}}
```

- 一行一个 JSON 对象，以换行结尾。
- stdout 只用于协议；非 JSON 行作为日志/错误展示并跳过。
- stderr 由核心透传给 UI；核心不落盘、不轮转，插件如需长期日志自行写入。
- 错误码建议：`not_supported`、`timeout`、`auth_required`、`rate_limited`、`invalid_response`、`failed`。

## 7. 数据结构

### Track

```json
{
  "platform": "youtube",
  "source_id": "abc123",
  "title": "Song",
  "artist": "Artist",
  "album": "Album",
  "duration": 240.5,
  "cover_url": "https://...",
  "year": 2020,
  "track_number": 3,
  "disc_number": 1
}
```

### StreamDescriptor

v1 只支持 pipe：

```json
{
  "type": "pipe",
  "cmd": ["yt-dlp", "-o", "-", "https://..."]
}
```

- 插件返回描述符后退出。
- 核心启动 `cmd`，读取 stdout 写入缓存。
- 不返回 URL/headers；headers、cookies、认证全部由插件/命令处理。
- 暂停播放不 kill 下载；切歌不 kill 上一个 instance，让它继续完成并缓存。
- 当前实现：全局同时最多 3 个活跃流；达到上限时新流等待旧流完成，不取消已有 instance；旧 instance 自然完成。
- 应用退出时统一 kill 所有流进程。
- 待启动队列和令牌桶限速尚未实现，留作后续优化。

### Lyrics

```json
{
  "text": "...",
  "format": "lrc",
  "language": "zh"
}
```

或 `result: null` 表示无歌词。

## 8. argv 配置

有效 argv 按层级解析：

```text
route.argv       (platform + capability)
platform.argv    (platform)
plugin.argv      (插件默认)
```

- 空值继承上一层。
- 非空 = 完整替换，不追加。
- 每行一个参数，保存为 `string[]`。
- 工作目录是插件目录。
- 不捆绑运行时，`node`/`python3` 走系统 PATH；启动失败时展示 OS 错误。
- 配置持久化在 SQLite 的 `plugin_config` 设置项中，包含 per-plugin 设置和 `platform -> capability -> provider` 列表。

## 9. 路由

- 路由键：`platform + capability`。
- 值：有序的 provider 列表。
- 每个平台每个能力排他：只选当前 provider，不合并、不并行查询、不做全平台聚合。
- 当前 provider 失败/超时/非零退出时，按顺序尝试下一个。
- 空结果不算失败，不回退。
- 所有 provider 失败时返回明确错误。
- 路由顺序、完整 provider 列表和 argv 覆盖持久化到 SQLite。
- 禁用/缺失的 provider 在运行时跳过。

## 10. 管理

- 安装：把插件目录放进插件目录（含 `manifest.json`），点“重新扫描”即可热加载，无需重启。
- 默认插件目录按平台解析：Linux/macOS 配置目录下的 `vide/plugins`，Windows 的 APPDATA 下 `vide/plugins`。
- 依赖：用户自行解决。
- 新插件默认启用。
- 插件管理页展示：名称、版本、描述、能力、启用开关、状态（idle / active N / last exit code）、argv 配置、当前 stderr/非 JSON stdout。
- 插件日志文件由插件自己管理；核心不提供日志轮转/保留设置，也不记录 argv。
- 不做签名、沙箱、市场；安全由用户负责。

## 11. 路由设置

- 按平台分组，每个能力一行。
- 用户从已安装且启用的插件中添加 provider、拖拽排序、删除。
- 每一行可以覆盖该路由的 argv。
- 空 = 继承平台/插件 argv。

## 12. 移除 M3U

- 删除 M3U/M3U8 导入、导出、嗅探和解析。
- 保留 playlist/entries 数据模型，用于在线歌单和用户歌单。
- HLS 由插件/yt-dlp 处理成 pipe，核心不解析 M3U8。

## 13. 内置适配器

- `plugins/meting-adapter`：netease / tencent / kugou / kuwo，提供 search、resolve、stream、playlist、lyrics（Kuwo 歌词为空）。需要 Node.js 18+。
- `plugins/ytdlp-adapter`：youtube / bilibili / soundcloud，提供 search、resolve、stream、playlist；YouTube/Bilibili 可从字幕生成歌词。需要 Python 3 + yt-dlp。
- 音频源：NetEase 和 Kuwo 可直接播放；Tencent/Kugou 通常需要登录 cookie，通过插件 argv 的 `--cookie` 传入。Bilibili/YouTube/SoundCloud 由 yt-dlp 处理。
- Migu/Baidu 不提供。

## 14. 与当前实现的差异

实现时需要改：

- 删除常驻进程 map、start_all/stop_all、长连接 stdin/stdout 循环。
- 每次调用 spawn 一次，写一行、读一行、退出/宽限 kill。
- 适配器 manifest 将 `metadata` 改为 `resolve`。
- 流描述符统一为 `{"type":"pipe","cmd":[...]}`。
- 删除 `search_online` 的无平台全平台聚合分支。
- 路由持久化保存完整顺序，而不是只保存当前 provider。
