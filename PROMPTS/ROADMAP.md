# Vide 开发路线指南

> 深度优先 · 模块化 · 可增量验证
>
> 本文档是 SPEC 的实现索引，不重复规范内容。阅读时请始终对照 `SPEC.md` / `SPEC_CN.md`。

---

## 目录结构总览

```
src-tauri/
├── Cargo.toml
├── src/
│   ├── main.rs                    # Tauri 入口
│   ├── lib.rs                     # crate root，模块声明
│   │
│   ├── models/                    # 阶段 1：核心类型
│   │   ├── mod.rs
│   │   ├── song.rs
│   │   ├── tag.rs
│   │   ├── playlist.rs
│   │   └── cache.rs
│   │
│   ├── infra/                     # 阶段 2：基础设施
│   │   ├── mod.rs
│   │   ├── db.rs
│   │   ├── hash.rs
│   │   ├── fs.rs
│   │   ├── config.rs
│   │   ├── logging.rs
│   │   ├── ipc.rs
│   │   └── process.rs
│   │
│   ├── features/                  # 阶段 3：功能特性
│   │   ├── mod.rs
│   │   ├── metadata.rs
│   │   ├── tags.rs
│   │   ├── search/
│   │   │   ├── mod.rs
│   │   │   ├── grammar.pest       # PEG 语法文件
│   │   │   ├── parser.rs
│   │   │   └── fuzzy.rs
│   │   ├── playlist.rs
│   │   ├── cache.rs
│   │   ├── plugin/
│   │   │   ├── mod.rs
│   │   │   ├── manifest.rs
│   │   │   ├── router.rs
│   │   │   ├── runtime.rs
│   │   │   └── types.rs
│   │   ├── player.rs
│   │   ├── library.rs
│   │   ├── network/
│   │   │   ├── mod.rs
│   │   │   ├── api.rs
│   │   │   ├── sse.rs
│   │   │   └── sync.rs
│   │   └── ui.rs
│   │
│   └── app.rs                     # 应用状态聚合 (Tauri State)
```

---

## 阶段 1：核心骨架与类型（Models & Types）

> 纯数据定义层，零外部依赖（仅 serde）。目标：定义整个系统的"语言"。

### 1.1 `models/song.rs` — 歌曲实体

SPEC §3.1–3.4 的类型映射：

| SPEC 概念 | Rust 类型 | 备注 |
|-----------|-----------|------|
| Song Entity | `Song` struct | 不可变，id 为 Deterministic ID |
| `id` | `SongId(String)` | newtype，内部是 hex hash |
| `sources` | `Vec<Source>` | `{ platform, id }` 二元组 |
| `paths` | `Vec<Utf8PathBuf>` | 路径数组，非标识 |
| `metadata` | `SongMetadata` | 扩展属性的容器 |

需要同时导出的类型：
- `Source { platform: String, id: String }` — §3.4
- `SongMetadata` — §4.1 内置属性 + §4.2 用户覆盖字段
- `ImportInfo` — §4.1 的 `import_order` / `import_source`，仅导入曲目携带

**关键约束：** Song 是 immutable object。更新通过「生成新 Song」而非原地修改。

### 1.2 `models/tag.rs` — 标签体系

SPEC §5.1–5.2：

```rust
// namespace:value 结构
Tag {
    namespace: String,  // e.g. "country", "genre"
    value: String,      // e.g. "russia", "pop"
}

// 元标签：用于 UI 分组，不参与查询
MetaTag {
    namespace: String,
    display_name: String,
    children: Vec<String>,  // 该命名空间下的可选值
}
```

Tag 系统在阶段 3 的 `features/tags.rs` 中实现业务逻辑，此处仅定义类型。

### 1.3 `models/playlist.rs` — 播放列表/导入

SPEC §7.2：

```rust
Playlist {
    id: PlaylistId,
    name: String,
    entries: Vec<PlaylistEntry>,
    source: PlaylistSource,  // M3U import / user-created
}

PlaylistEntry {
    song_id: SongId,
    import_order: Option<u32>,  // 仅导入条目
}

PlaylistSource::M3U { file_path: PathBuf }
PlaylistSource::UserCreated
```

### 1.4 `models/cache.rs` — 缓存记录

SPEC §8.2–8.4：

```rust
CacheEntry {
    song_id: SongId,
    path: PathBuf,       // cache/abc123.media
    size: u64,
    source: String,      // "youtube" 等
    last_used: SystemTime,
    valid: bool,
}
```

### 1.5 `lib.rs` — crate root

声明模块树，导出 `pub use models::*`。此时项目应能 `cargo check` 通过。

### 阶段 1 完成标志

- [ ] 所有核心类型定义完成
- [ ] `cargo check` 通过
- [ ] 每个类型有 `#[derive(Serialize, Deserialize, Clone, Debug)]`
- [ ] `SongId` 的 newtype pattern 就位

---

## 阶段 2：基础设施层（DB & IPC 通信）

> 提供所有功能特性共用的底层能力。每个模块可独立测试。

### 2.1 `infra/hash.rs` — Deterministic ID 生成

SPEC §3.2：

```
在线来源：hash(platform + source_id)
本地文件：hash(size + mtime + first_chunk + last_chunk)
```

- 哈希算法：blake3（快速、确定性、跨平台一致）
- 暴露 `fn song_id_from_source(platform: &str, source_id: &str) -> SongId`
- 暴露 `fn song_id_from_file(path: &Path) -> Result<SongId>` — 需要读取文件头尾 chunk
- 暴露 `fn quick_hash(path: &Path) -> Result<QuickHash>` — 仅 size+mtime，用于扫描时快速比对

**与 SPEC 对齐注意：** 英文 SPEC 明确说 "Compute lazily during scanning"，所以 quick_hash 和 full content hash 是两个不同路径。

### 2.2 `infra/db.rs` — SQLite 元数据层

SPEC §8.1 + §15.2（配置也存 SQLite）：

核心职责：
- 连接管理（WAL mode，single connection pool）
- Schema 迁移（使用 `rusqlite` + 手写 migration 或 `refinery`）
- 基础 CRUD：`Song`、`Tag`、`Playlist`、`CacheEntry`、`Setting`

表设计概要（非规范内容，实现参考）：

```
songs          — song_id PK, title, artist, album, duration, added_time, ...
song_paths     — song_id FK, path PK
song_sources   — song_id FK, platform, source_id
tags           — song_id FK, namespace, value
playlists      — playlist_id PK, name, source_type, source_path
playlist_entries — playlist_id FK, song_id FK, import_order
cache_entries  — song_id FK, path, size, source, last_used, valid
settings       — key PK, value JSON
```

关键接口：
- `fn upsert_song(song: &Song) -> Result<()>`
- `fn get_song(id: &SongId) -> Result<Option<Song>>`
- `fn query_songs(filter: &QueryFilter) -> Result<Vec<Song>>` — 给搜索模块用
- `fn update_setting(key: &str, value: &serde_json::Value) -> Result<()>`
- `fn get_setting(key: &str) -> Result<Option<serde_json::Value>>`

### 2.3 `infra/fs.rs` — 文件系统工具

SPEC §16.1–16.2（库扫描需要的 fs 工具）+ §8.2（缓存目录管理）：

- `fn scan_directory(root: &Path, extensions: &[&str]) -> impl Iterator<Item = DirEntry>` — 递归遍历
- `fn ensure_cache_dir(base: &Path) -> Result<PathBuf>` — 创建 `cache/` 结构
- `fn read_first_last_chunks(path: &Path, chunk_size: usize) -> Result<(Vec<u8>, Vec<u8>)>` — 给 hash 用

### 2.4 `infra/config.rs` — 配置管理

SPEC §15.2：

- `AppConfig` struct — 对应 settings 表
- `fn load(db: &Db) -> Result<AppConfig>`
- `fn save(db: &Db, config: &AppConfig) -> Result<()>`
- 包含字段：`library_roots: Vec<PathBuf>`、`plugin_dir: PathBuf`、`concurrency_limits: HashMap<String, u32>`

### 2.5 `infra/logging.rs` — 结构化日志

SPEC §15.3：

- 使用 `tracing` + `tracing-subscriber`
- 初始化函数：`fn init_logging(level: LevelFilter)`
- 插件 stderr 在此层被忽略（`process.rs` 负责）

### 2.6 `infra/ipc.rs` — 插件 JSON lines 协议

SPEC §10.1 的 Rust 侧实现：

```rust
// 核心 → 插件
PluginRequest {
    id: String,        // e.g. "t123"
    action: String,    // "search" | "resolve" | "stream"
    params: Value,     // JSON object
}

// 插件 → 核心
PluginResponse {
    id: String,
    result: Option<Value>,
    error: Option<String>,
}
```

- `fn write_request(writer: &mut impl Write, req: &PluginRequest) -> Result<()>`
- `fn read_response(reader: &mut impl BufRead) -> Result<PluginResponse>`
- 严格的 JSON lines 语义：每行一个 JSON，非 JSON 行直接忽略

### 2.7 `infra/process.rs` — 子进程管理

SPEC §11（进程管理）+ §9.2（插件运行时）：

核心职责：
- 启动子进程：`fn spawn_plugin(exec: &[String]) -> Result<ChildProcess>`
- 超时控制：per-action timeout（search: 10s, stream: 30s, resolve: 5s）
- 取消机制：SIGTERM → timeout → SIGKILL
- 并发限制：Semaphore per plugin（防止 LX Music 式请求风暴）
- TaskTable：跟踪所有 in-flight 操作

```rust
struct ProcessManager {
    tasks: HashMap<String, TaskInfo>,
    semaphores: HashMap<String, Arc<Semaphore>>,  // per-plugin
}

struct TaskInfo {
    plugin: String,
    child: Child,
    status: TaskStatus,
    started_at: Instant,
}
```

### 阶段 2 完成标志

- [ ] SQLite schema migration 可运行
- [ ] 能通过 `db.rs` 存取 `Song` 和 `Playlist`
- [ ] `hash.rs` 的 deterministic ID 可通过单元测试验证（同一输入始终相同输出）
- [ ] `process.rs` 能启动一个简单子进程、读取 stdout、超时终止
- [ ] `ipc.rs` 的 JSON lines 读写可独立测试

---

## 阶段 3：独立功能特性（Features）

> 每个 feature 是一个独立可测试的模块。按依赖深度排序，从无依赖到高依赖。

### 3.1 `features/metadata.rs` — 元数据管理

依赖：`models` + `infra/db`

SPEC §4。核心是 db 之上的薄封装：
- 解析本地文件内嵌元数据（使用 `lofty` crate 读取 ID3/Vorbis/MP4 tags）
- 将解析结果映射到 `SongMetadata`
- 用户覆盖写入（§4.2 的 `display_title`、`artist_override`）

**不在此模块处理：** 远程来源的元数据（由 plugin 模块的 resolve action 处理）。

### 3.2 `features/tags.rs` — 标签业务逻辑

依赖：`models` + `infra/db`

SPEC §5。在 db CRUD 之上的业务层：
- 添加/移除标签
- 构建元标签树（用于 UI 展示）
- 标签命名空间枚举（从 db 聚合 distinct namespaces）

### 3.3 `features/search/` — 搜索系统

依赖：`models` + `infra/db` + `features/tags`

SPEC §6。这是最复杂的独立模块：

**`grammar.pest`** — PEG 语法定义（§6.2.1）：
```pest
query = { SOI ~ expr ~ EOI }
expr = { or_expr }
or_expr = { and_expr ~ ("OR" ~ and_expr)* }
and_expr = { atom ~ ("AND" ~ atom)* }
atom = { paren_expr | not_expr | filter }
paren_expr = { "(" ~ expr ~ ")" }
not_expr = { ("NOT" | "!") ~ atom }
filter = { tag_filter | text_filter | cmp_filter }
tag_filter = { namespace ~ ":" ~ value }
text_filter = { field ~ "CONTAINS" ~ quoted_string }
cmp_filter = { field ~ cmp_op ~ value }
// ...
```

**`parser.rs`** — 使用 `pest` + `pest_derive` 解析查询，输出 `QueryAst`

**`fuzzy.rs`** — §6.3 自动补全 + §6.4 模糊搜索：
- `unicode-normalization`（NFD 分解处理重音字符）
- `fuzzy-matcher`（容错匹配）
- FuzzyScore 排序

**`mod.rs`** — 组合层：
- `fn search(db: &Db, query: &str) -> Result<Vec<Song>>` — 主搜索入口
- `fn fuzzy_search(db: &Db, text: &str) -> Result<Vec<Song>>` — 次级搜索
- `fn autocomplete(db: &Db, prefix: &str) -> Vec<Suggestion>`

### 3.4 `features/playlist.rs` — 播放列表

依赖：`models` + `infra/db` + `features/metadata`

SPEC §7.2：
- M3U/M3U8 解析（使用 `m3u` crate，或 `kdam` 等）
- 内容嗅探区分 M3U 和 HLS manifest（§7.2 的关键要求）
- 导入流程：解析 → 映射 song_id → 创建 playlist entries
- 导出：song_id → metadata snapshot → 格式化输出

### 3.5 `features/cache.rs` — 媒体缓存

依赖：`models` + `infra/db` + `infra/fs`

SPEC §8.2–8.4：
- 缓存写入：`fn cache_media(song_id: &SongId, data: impl Read, source: &str) -> Result<PathBuf>`
- 缓存读取：`fn get_cached(song_id: &SongId) -> Result<Option<PathBuf>>`
- 淘汰逻辑：磁盘压力 → 损坏检测 → LRU fallback
- 固定/刷新 API

### 3.6 `features/plugin/` — 插件系统

依赖：`models` + `infra/*`（几乎全部基础设施）

SPEC §9 + §10。这是最核心的架构模块：

**`types.rs`** — §9.3 清单 + §9.5 能力类型：
```rust
CapabilityType::Search | Metadata | Playback | Lyrics | Recommendation

PlatformCapability {
    capability: CapabilityType,
    platform: String,
}

PluginManifest {
    name: String,
    version: String,
    exec: Vec<String>,
    capabilities: Vec<PlatformCapability>,
}
```

**`manifest.rs`** — 清单加载：
- 扫描 plugin_dir 下的所有 manifest.json
- 解析并验证

**`router.rs`** — §9.4 + §9.6 路由与编排：
- `RoutingTable: HashMap<(Platform, Capability), Vec<Provider>>`
- 启动时：manifest → merge with saved routing config → build runtime table
- 路由持久化到 SQLite settings 表
- 每个 (platform, capability) 排他：只选一个 provider（§9.7）

**`runtime.rs`** — 插件进程生命周期：
- 启动 manifest 声明的子进程
- 发送请求、读取响应（通过 `infra/ipc.rs`）
- 超时、取消、错误处理（通过 `infra/process.rs`）
- 路由分发：`fn dispatch_search(platform: &str, query: &str) -> Result<Vec<SearchResult>>`

### 3.7 `plugins/ytdlp-adapter/` — yt-dlp 适配插件（非 Rust）

依赖：无（独立进程，通过 JSON lines 与核心通信）

SPEC §12。**这是一个插件，不是 Rust 代码。** 核心不需要知道 yt-dlp 的存在。

插件目录结构：

```
plugins/ytdlp-adapter/
├── manifest.json          # 声明 capabilities
├── adapter.py             # Python 适配器（yt-dlp 本身就是 Python）
└── requirements.txt
```

`manifest.json`：

```json
{
  "name": "ytdlp-adapter",
  "version": "1.0",
  "exec": ["python3", "adapter.py"],
  "capabilities": [
    { "type": "search", "platform": "youtube" },
    { "type": "search", "platform": "bilibili" },
    { "type": "metadata", "platform": "youtube" },
    { "type": "metadata", "platform": "bilibili" },
    { "type": "stream", "platform": "youtube" },
    { "type": "stream", "platform": "bilibili" }
  ]
}
```

适配器职责：
- 接收 `search` action → 调用 `yt-dlp --flat-playlist -j "ytsearch:$q"`
- 接收 `metadata` action → 调用 `yt-dlp -j $url`（§12.4 元数据模式）
- 接收 `stream` action → 返回 pipe 命令 `{ "cmd": ["yt-dlp", "-o", "-", "$url"], "mode": "stdout" }`（§12.3）
- 实现分层策略（§12.5）：优先返回 URL+headers，回退 pipe command

**核心侧无需任何 yt-dlp 特定代码。** 路由完全由 `features/plugin/router.rs` 处理。

### 3.8 `features/player.rs` — 播放引擎

依赖：`features/plugin` + `features/cache` + `infra/process`

SPEC §12.2（格式支持）+ §11（进程管理）：

- 封装 mpv 作为播放后端（通过 IPC 或 subprocess）
- 处理 StreamDescriptor 的两种类型（§10.3）：
  - `url_stream` → 直接传 URL 给 mpv
  - `stream` (pipe) → 启动子进程，将 stdout pipe 给 mpv
- 播放状态管理：playing / paused / stopped
- 进度追踪

**注意：** player 不直接调用 yt-dlp。它只消费 plugin router 返回的 StreamDescriptor。yt-dlp 的复杂性被插件层完全隔离。

### 3.9 `features/library.rs` — 库扫描与去重

依赖：`infra/db` + `infra/hash` + `infra/fs` + `features/metadata`

SPEC §16：

- 扫描流程（§16.2）：遍历 library_roots → quick_hash → 查 db → upsert or create
- 去重（§16.3）：hash 匹配 → 合并 paths 数组
- 增量扫描：比较 mtime 避免全量重扫
- 本地文件播放回退（§16.4）：paths 数组顺序 → 依次尝试

### 3.10 `features/network/` — 网络与远程共享

依赖：`infra/db` + `features/cache` + `features/plugin` + `features/player`

SPEC §13 + §14。使用 `axum` 或 `warp`：

**`api.rs`** — §13.2 HTTP API：
- `GET /song/{id}` → octet-stream
- `GET /meta/{id}` → JSON
- `GET /cover/{id}` → image
- `GET /search?q=...` → 转发到 plugin router
- `GET /status` + `GET /status/stream` (SSE)

**`sse.rs`** — §13.3 Server-Sent Events：
- 播放状态变更推送
- `track_switching` 事件

**`sync.rs`** — §14.4 同步：
- Diff-based 元数据复制（非全量转储）
- 节点发现与连接

### 3.11 `features/ui.rs` — UI 状态管理

依赖：几乎所有 features

SPEC §15。Tauri commands 的 Rust 侧 handler：
- 每个 Tauri command 对应一个 handler function
- 应用状态通过 Tauri State 注入
- Provider 状态显示（§15.5）

### 3.12 `app.rs` — 应用状态聚合

将所有模块组装为 Tauri 应用：

```rust
struct AppState {
    db: Db,
    config: AppConfig,
    process_manager: ProcessManager,
    router: RoutingTable,
    cache: CacheManager,
    // ...
}
```

- 初始化顺序：config → db → logging → plugin scan → router build → http server → tauri setup

### 阶段 3 完成标志

- [ ] 能扫描本地目录并入库
- [ ] 能通过结构化查询搜索歌曲
- [ ] 能导入 M3U 播放列表
- [ ] 能通过 yt-dlp 插件搜索在线平台
- [ ] 能播放本地文件和在线流
- [ ] HTTP API 可访问，SSE 状态推送工作
- [ ] Tauri UI 可连接并显示数据

---

## 依赖关系图

```
阶段 1 (纯类型)
  models/  ← 无外部依赖

阶段 2 (基础设施)
  hash    ← blake3
  db      ← rusqlite, serde_json
  fs      ← std::fs
  config  ← db
  logging ← tracing
  ipc     ← serde_json
  process ← tokio

阶段 3 (功能特性)
  metadata   ← models, db
  tags       ← models, db
  search     ← models, db, tags, pest
  playlist   ← models, db, metadata, m3u crate
  cache      ← models, db, fs
  plugin     ← models, infra/*, process, ipc
  provider   ← plugin, process
  player     ← plugin, cache, process
  library    ← db, hash, fs, metadata
  network    ← db, cache, plugin, player, axum
  ui         ← 全部
  app        ← 全部
```

---

## 开发顺序（推荐）

```
Week 1-2:  阶段 1 + 2.1 (hash) + 2.2 (db)
           → 能存取 Song，能生成 Deterministic ID

Week 3-4:  2.3 (fs) + 3.9 (library) + 3.1 (metadata)
           → 能扫描本地目录并入库

Week 5-6:  3.2 (tags) + 3.3 (search)
           → 能打标签、结构化搜索

Week 7-8:  2.6 (ipc) + 2.7 (process) + 3.6 (plugin)
           → 插件系统骨架就位

Week 9-10: plugins/ytdlp-adapter (Python) + 3.8 (player)
           → 编写 yt-dlp 适配插件，能搜索在线平台并播放

Week 11:   3.4 (playlist) + 3.5 (cache)
           → M3U 导入 + 缓存管理

Week 12:   3.10 (network) + 3.11 (ui) + 3.12 (app)
           → HTTP API + Tauri UI 集成
```

---

## 关键 crate 依赖

| crate | 用途 | 对应 SPEC |
|-------|------|----------|
| `rusqlite` | SQLite (WAL mode) | §8.1 |
| `blake3` | Deterministic ID hashing | §3.2 |
| `serde` / `serde_json` | 序列化 | 全局 |
| `tokio` | 异步运行时 | §17 构建顺序 |
| `pest` / `pest_derive` | PEG parser | §6.2.1 |
| `lofty` | 本地文件元数据解析 | §4 |
| `tracing` / `tracing-subscriber` | 结构化日志 | §15.3 |
| `axum` | HTTP API | §13.2 |
| `fuzzy-matcher` | 模糊搜索 | §6.3 |
| `unicode-normalization` | 重音字符匹配 | §6.3 |

> **注意：** yt-dlp 不在 Rust 依赖中。它作为外部插件运行，通过 JSON lines 协议与核心通信（§9.1）。核心侧只实现通用的插件协议和路由，不包含任何 yt-dlp 特定代码。

---

## 验证策略

每个阶段完成时的最小可验证路径：

| 阶段 | 验证方式 |
|------|----------|
| 1 | `cargo check` + 单元测试（Song 序列化/反序列化） |
| 2 | 集成测试（db 读写、hash 一致性、process 超时终止） |
| 3 | 端到端冒烟测试（扫描 → 搜索 → 播放本地文件） |

---

*路线图结束。*
