# Vide 项目进度总览

> 本文档是项目的完整上下文快照。适用于新对话加载后快速了解项目全貌。
> 对照阅读：`SPEC_CN.md`（设计规范）、`ROADMAP.md`（路线图）

---

## 1. 项目概况

- **名称**: Vide
- **类型**: 跨平台音乐平台（桌面优先）
- **技术栈**: Rust (核心) + Tauri 2 (桌面壳) + Svelte (前端)
- **元数据库**: SQLite (WAL mode) via SeaORM
- **插件通信**: JSON lines over stdin/stdout
- **插件语言**: 任意 (通过子进程)

---

## 2. 目录结构与模块依赖

```
src-tauri/src/
├── main.rs              # Tauri 入口（不变）
├── lib.rs               # crate root → 模块声明 + Tauri run()
│
├── models/              # Phase 1: 纯数据定义层 (serde only)
│   ├── song.rs          #   Song, SongId, Source, SongMetadata
│   ├── tag.rs           #   Tag (namespace:value), MetaTag
│   ├── playlist.rs      #   Playlist, PlaylistId, PlaylistEntry, PlaylistSource
│   └── cache.rs         #   CacheEntry
│
├── infra/               # Phase 2: 基础设施层
│   ├── hash.rs          #   blake3 确定性ID生成
│   ├── fs.rs            #   目录遍历 + 缓存目录 + 文件chunk读取
│   ├── logging.rs       #   tracing-subscriber 初始化
│   ├── ipc.rs           #   JSON lines 协议 (PluginRequest/Response)
│   ├── db.rs            #   SeaORM 连接管理 + 8表 CRUD + Schema迁移
│   ├── config.rs        #   AppConfig 读写 settings 表
│   └── process.rs       #   子进程管理器 (Semaphore + TaskTable)
│
├── features/            # Phase 3: 功能特性
│   ├── metadata.rs      #   lofty 解析本地文件 ID3/Vorbis/MP4 元数据
│   ├── tags.rs          #   标签CRUD + 命名空间聚合 + 元标签树
│   ├── search/
│   │   ├── grammar.pest #   PEG 查询语法
│   │   ├── parser.rs    #   QueryAst + pest 解析 + 内存求值器
│   │   └── fuzzy.rs     #   模糊搜索 + 评分 (exact > prefix > substring > Jaccard)
│   ├── playlist.rs      #   M3U 解析 + HLS 嗅探 + 导入/导出
│   ├── cache.rs         #   媒体缓存管理 (temp→rename, LRU淘汰)
│   ├── library.rs       #   库扫描 + 去重 + 播放回退
│   ├── plugin/          #   插件系统：类型/清单/路由/运行时
│   │   ├── types.rs     #   CapabilityType, PluginManifest, StreamDescriptor
│   │   ├── manifest.rs  #   扫描 plugin_dir/manifest.json
│   │   ├── router.rs    #   RoutingTable (platform,capability)→Provider
│   │   └── runtime.rs   #   tokio 子进程 + semaphore + 超时控制
│   ├── player.rs        #   mpv 子进程封装 (URL/direct + pipe)
│   ├── network/
│   │   ├── mod.rs       #   NetworkManager 启动/停止
│   │   ├── api.rs       #   axum 路由 (9 endpoints)
│   │   └── sse.rs       #   broadcast 频道 + SSE流
│   └── ui.rs            #   20个 Tauri command handlers
│
└── app.rs               # AppState 聚合 + 初始化编排
```

**模块依赖图（简化）**:
```
app.rs (AppState)
  ├── infra::db
  ├── infra::config
  ├── features::player
  ├── features::network
  └── features::plugin

features/* → infra/* → models/*
```

---

## 3. 关键设计决策

| 决策 | 方案 | 理由 |
|------|------|------|
| ORM | SeaORM 1.x | 异步、Entity 链式查询、无宏依赖的表定义、Send+Sync 连接 |
| 搜索语法 | pest PEG + 内存求值 | 编译期语法校验、桌面场景 <10万首歌内存过滤可接受 |
| 插件通信 | JSON lines stdin/stdout | 语言无关、简单可靠 |
| 播放后端 | mpv 子进程 | 成熟、不捆绑解码库 |
| HTTP | axum 0.7 | 最流行的异步 web 框架 |
| 状态共享 | Arc\<tokio::sync::Mutex\<T\>\> | Tauri State 要求 Send+Sync |

---

## 4. 数据库 Schema（8张表）

通过 SeaORM entity 定义 + `Schema::create_table_from_entity().if_not_exists()` 自动建表。

| 表 | 主键 | 说明 |
|----|------|------|
| `songs` | `song_id TEXT` | 歌曲核心字段 + metadata_json (JSON blob) |
| `song_paths` | `(song_id, path)` | 多路径存储 |
| `song_sources` | `(song_id, platform)` | 来源标识 |
| `tags` | `(song_id, namespace, value)` | namespace:value 标签 |
| `playlists` | `playlist_id TEXT` | M3U / UserCreated |
| `playlist_entries` | `(playlist_id, song_id)` | 有序引用 |
| `cache_entries` | `(song_id, path)` | 缓存文件追踪 |
| `settings` | `key TEXT` | JSON 值配置 |

Entities 定义: `src-tauri/src/infra/db/entity/*.rs`
Database 方法: `src-tauri/src/infra/db/mod.rs` — `Database` struct

---

## 5. Tauri 命令清单（20个）

```rust
// Library
list_songs()                 → Vec<Song>
get_song(id)                 → Option<Song>
search_songs(query)          → Vec<Song>          // PEG 结构化查询
scan_library()               → ScanResult

// Playback
get_player_status()          → PlaybackStatus
play_song(id)                → PlaybackStatus
stop_playback()              → PlaybackStatus

// Search
fuzzy_search_songs(query)    → Vec<FuzzyMatch>    // 容错搜索
autocomplete(prefix)         → Vec<String>         // 搜索栏补全

// Tags
get_meta_tags()              → Vec<MetaTag>        // 标签树
add_tag(song_id, ns, val)    → ()
remove_tag(song_id, ns, val) → ()
get_song_tags(song_id)       → Vec<Tag>
get_namespaces()             → Vec<String>

// Playlists
import_m3u(path)             → Playlist
get_playlist(id)             → Option<Playlist>
export_playlist(id, out)     → ()

// Server
start_server(addr)           → ()
stop_server()                → ()
get_server_status()          → bool
```

定义位置: `src-tauri/src/features/ui.rs`
注册位置: `src-tauri/src/lib.rs` — `tauri::generate_handler![]`

---

## 6. 初始化顺序 (`app.rs :: AppState::initialize()`)

1. Open database (create tables via schema migrations)
2. Load AppConfig from settings table
3. Init PluginManager if plugin_dir exists
4. Create PlayerManager (idle)
5. Create NetworkManager (not started)
6. Wrap everything in Arc/Mutex → AppState
7. Register commands → Tauri::Builder::run()

---

## 7. 插件系统

**架构**: 外部进程 ↔ stdin/stdout JSON lines ↔ Rust Core

**清单文件** (`plugin/<name>/manifest.json`):
```json
{
  "name": "yt-dlp",
  "version": "1.0",
  "exec": ["python3", "adapter.py"],
  "capabilities": [
    { "type": "search", "platform": "youtube" },
    { "type": "stream", "platform": "youtube" }
  ]
}
```

**IPC 协议**:
```
Core → Plugin: {"id":"t001","action":"search","params":{...}}
Plugin → Core: {"id":"t001","result":[...]}
Plugin → Core: {"id":"t001","error":"not_supported"}
```

**能力类型**: `search`, `metadata`, `playback`, `lyrics`, `recommendation`

**路由**: 每个 `(platform, capability)` 排他，回退顺序可配置。

**超时**: search 10s, stream 30s, resolve 5s。超时后 SIGKILL。

**相关代码**:
- 核心协议: `src-tauri/src/infra/ipc.rs`
- 插件类型: `src-tauri/src/features/plugin/types.rs`
- 清单加载: `src-tauri/src/features/plugin/manifest.rs`
- 路由表: `src-tauri/src/features/plugin/router.rs`
- 运行时代码: `src-tauri/src/features/plugin/runtime.rs`
- 编排: `src-tauri/src/features/plugin/mod.rs`

---

## 8. 编译 & 运行

```bash
cd src-tauri

# 检查
cargo check

# 测试 (16 tests, 0 failed)
cargo test

# 运行 (需要 Tauri 环境 + mpv)
cargo tauri dev

# 构建
cargo build
```

---

## 9. 当前 Warning 状态（20个，全部有意预留）

### A. 未来可串联的公共 API（8个）
| Warning | 原因 | 计划 |
|---------|------|------|
| `CacheManager` 全部方法 | 缓存模块已实现，播放器未接入 | 播放器实现后串联 |
| `update_from_file` | 内联提取而非使用此函数 | 重构为统一调用 |
| `find_first_available_path` | 播放回退工具 | 播放器集成 |
| `is_file_changed` | 增量扫描 | 库扫描优化时用 |
| `check_alive` | 定时轮询检查 | 前端定时调用 |

### B. 插件系统内部（10个）
| Warning | 原因 |
|---------|------|
| `PluginRuntime.ensure_running`, `send_request` 等 | 前端未暴露插件控制命令 |
| `TIMEOUT_*` 常量 | runtime 调用时使用 |
| `CapabilityType`, `SearchResultItem`, `PluginStatus` | 类型定义，用于插件结果 |

### C. AppState / PluginManager 预留字段（2个）
| Warning | 处理 |
|---------|------|
| `AppState.plugins` | 已加 `#[allow(dead_code)]` |
| `PluginManager.runtime` | 运行时内部字段 |

---

## 10. 已完成 vs 未完成

### 已完成 ✅

- Phase 1: 所有核心类型 (Song, Tag, Playlist, CacheEntry)
- Phase 2: 所有基础设施 (Hash, FS, Logging, IPC, DB/SeaORM, Config, Process)
- Phase 3: 全部 features 模块 (metadata, tags, search, playlist, cache, library, plugin, player, network, ui)
- SPEC §1-§13, §15-§16 覆盖
- 20个 Tauri 命令注册
- 16 个测试通过 (`cargo test`)

### 未完成/推迟 ❌

- **§14 分布式节点**: 跨设备同步 + 远程分享
- **§12 yt-dlp 适配器**: 非 Rust 插件，需单独开发
- **前端 UI (Svelte)**: 项目中的 `src/routes/` 和 `src/app.html` 还是 Tauri 模板
- **插件文档**: 开发者文档，包括 manifest.json 规格和 IPC 协议说明

---

## 11. 推荐后续路线（Phase 4）

| 优先级 | 工作 | 估算 | 依赖 |
|--------|------|------|------|
| **P0** | 前端 UI (Svelte) | 大 | 全部 Rust 后端 |
| **P1** | 插件文档 + yt-dlp 适配器 | 中 | 插件系统已就绪 |
| **P2** | 分布式节点 | 大 | 基础架构已就绪 |
| **P3** | 移动端支持 | 待定 | Tauri Mobile 成熟 |
| **P4** | 插件市场 | 远期 | 先有插件生态 |

---

## 12. 关键依赖版本

```toml
tauri = "2"
sea-orm = "1" (sqlx-sqlite + macros + runtime-tokio)
tokio = "1" (process + sync + time + rt + macros)
axum = "0.7"
tower-http = "0.5" (cors)
pest = "2" + pest_derive
lofty = "0.21"
blake3 = "1"
walkdir = "2"
dirs = "5"
mime_guess = "2"
tokio-util = "0.7"
tokio-stream = "0.1"
```

---

*文档生成日期: 2026-06-20*
*最后验证: `cargo check` 通过, `cargo test` 16/16 通过, 20 warnings (全部有意)*
