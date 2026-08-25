# Vide 项目进度总览

> 最后更新: 2026-06-30
> 对照阅读：`SPEC_CN.md`（设计规范）、`ROADMAP.md`（路线图）

---

## 1. 项目概况

- **名称**: Vide
- **类型**: 跨平台音乐平台（桌面优先）
- **技术栈**: Rust (核心) + Tauri 2 (桌面壳) + Svelte 5 (前端)
- **元数据库**: SQLite (WAL mode) via SeaORM
- **插件通信**: JSON lines over stdin/stdout
- **插件语言**: 任意 (通过子进程)

---

## 2. 已完成功能

### 后端 (Rust)

| 功能 | 状态 | 说明 |
|------|------|------|
| **数据模型** | ✅ | Song, Tag, Playlist, CacheEntry 等 |
| **数据库 (SeaORM)** | ✅ | 8表 CRUD，WAL 模式 |
| **文件哈希** | ✅ | blake3 确定性 ID 生成 |
| **文件系统** | ✅ | 目录扫描、缓存目录、文件 chunk 读取 |
| **配置系统** | ✅ | AppConfig 读写 settings 表 |
| **IPC 协议** | ✅ | JSON lines PluginRequest/Response |
| **进程管理** | ✅ | Semaphore + TaskTable |
| **元数据解析** | ✅ | lofty 解析 ID3/Vorbis/MP4 |
| **标签系统** | ✅ | 标签 CRUD + 命名空间聚合 |
| **搜索** | ✅ | pest PEG 解析器 + 模糊搜索 + 自动补全 |
| **M3U 解析** | ✅ | 导入 + HLS 嗅探 |
| **缓存管理** | ✅ | temp→rename，LRU 淘汰 |
| **库扫描** | ✅ | 去重 + 播放回退 |
| **播放器 (mpv)** | ✅ | URL 播放 + Pipe 模式 + Seek |
| **网络服务** | ✅ | axum 9 端点 API + SSE |
| **插件系统** | ✅ | 编排层：search / resolve / stream + 回退机制 |
| **系统托盘** | ✅ | 显示/隐藏 + 菜单 + 左键切换 |
| **`search_online` 命令** | ✅ | 通过插件搜索网络内容 |
| **`resolve_song` 命令** | ✅ | 解析在线歌曲元数据，存入 DB |
| **`get_lyrics` 命令** | ✅ | 获取歌词 |
| **`list_library_songs` 命令** | ✅ | 过滤出"我的库"（有路径或有标签） |
| **`list_playlists` 命令** | ✅ | 列出所有播放列表 |
| **进度条 Seek** | ✅ | mpv stdin 发送 `set time-pos` |

### 前端 (Svelte)

| 功能 | 状态 | 说明 |
|------|------|------|
| **无边框窗口** | ✅ | 自定义标题栏 + Windows 风格控制按钮 |
| **窗口大小持久化** | ✅ | localStorage 保存/恢复尺寸和最大化状态 |
| **侧栏导航** | ✅ | 搜索/列表/设置 三个按钮 |
| **歌曲列表** | ✅ | 可配置排序、列头拖拽、列显隐 |
| **本地搜索** | ✅ | 客户端即时过滤 title/artist/album/paths |
| **搜索历史** | ✅ | localStorage 持久化，最近 20 条 |
| **插件搜索下拉 (标题栏)** | ✅ | Tab 切换平台，点击即播放 |
| **播放栏** | ✅ | 显示当前歌曲、时间、播放/暂停 |
| **进度条** | ✅ | 可点击 seek |
| **全屏播放详情** | ✅ | 封面 + 歌词 + 进阶设置 + 控制 |
| **歌词显示** | ✅ | 从后端获取并显示 |
| **标签编辑悬浮窗** | ✅ | 按命名空间分组 + 增删 + 自动补全 |
| **右键菜单** | ✅ | 歌曲行右键：播放/编辑标签/下载(占位) |
| **导入 Modal** | ✅ | 本地 M3U 导入 + 在线链接（占位） |
| **播放列表页** | ✅ | 显示已有列表 |
| **设置页** | 🔶 半完成 | 库管理、数据导出、关于（库目录增删/扫描缺 onclick） |
| **JSON 导出** | ✅ | 全部歌曲导出为 JSON |

### 插件系统

| 组件 | 状态 | 路径 |
|------|------|------|
| PluginManager 编排 | ✅ | `features/plugin/mod.rs` |
| 路由表 (Mutex) | ✅ | `features/plugin/router.rs` |
| 运行时 + 超时 | ✅ | `features/plugin/runtime.rs` |
| 类型定义 | ✅ | `features/plugin/types.rs` |
| **yt-dlp 适配器** | ✅ 已创建 | `plugins/ytdlp-adapter/`（需安装 yt-dlp） |

---

## 3. Tauri 命令清单（共 24 个）

```rust
// Library (4)
list_songs()              → Vec<Song>            // 全量
list_library_songs()      → Vec<Song>            // 我的库（有路径/标签）
get_song(id)              → Option<Song>
search_songs(query)       → Vec<Song>
scan_library()            → ScanResult

// Playback (4)
get_player_status()       → PlaybackStatus
get_lyrics(song_id)       → Option<String>
play_song(id)             → PlaybackStatus
stop_playback()           → PlaybackStatus
seek(position_secs)       → PlaybackStatus

// Online Search (2)
search_online(query, platform?) → Vec<SearchResultItem>
resolve_song(platform, id)      → Song

// Search (2)
fuzzy_search_songs(query) → Vec<FuzzyMatch>
autocomplete(prefix)      → Vec<String>

// Tags (4)
get_meta_tags()           → Vec<MetaTag>
add_tag(song_id, ns, val) → ()
remove_tag(song_id, ns, val) → ()
get_song_tags(song_id)    → Vec<Tag>
get_namespaces()          → Vec<String>

// Playlists (3)
import_m3u(path)          → Playlist
get_playlist(id)          → Option<Playlist>
list_playlists()          → Vec<Playlist>

// Server (3)
start_server(addr)        → ()
stop_server()             → ()
get_server_status()       → bool
```

---

## 4. 当前已知 Bug

| # | Bug | 严重程度 | 文件 | 说明 |
|---|-----|----------|------|------|
| 1 | **设置页"添加"按钮无 onclick** | 🔴 | `settings/+page.svelte:45` | 点击无反应 |
| 2 | **设置页"立即扫描"按钮无 onclick** | 🔴 | `settings/+page.svelte:51` | 需调用 `invoke("scan_library")` |
| 3 | **export_playlist 仍是 M3U 格式** | 🟡 | `ui.rs:194` | 用户要求废弃 M3U 导出 |
| 4 | **搜索历史有时不显示** | 🟡 | `+page.svelte` | `handleSearchBlur` 延迟 200ms 可能不够 |
| 5 | **新用户空状态引导不足** | 🟡 | `+page.svelte` | 空状态提示应引导到设置页配置库目录 |
| 6 | **插件搜索无插件时报错信息不友好** | 🟡 | `TitleBar.svelte` | "plugin system not available" 对用户无意义 |
| 7 | **离线使用 `search_online` 报错未处理** | 🟡 | `TitleBar.svelte` | 网络错误应提示而非 crash |

---

## 5. 待完成任务

| 优先级 | 任务 | 类型 | 估算 |
|--------|------|------|------|
| **P0** | Bug 修复（见上表） | 混合 | 小 |
| **P1** | 全局快捷键 | 后端+前端 | 中 |
| **P2** | 设置页完善（库目录管理、插件配置） | 前端+后端 | 中 |
| **P3** | 播放详情进阶设置（速度、均衡器） | 前端+后端 | 大 |
| **P4** | 在线导入（bilibili/YouTube 播放列表） | 后端+插件 | 中 |
| **P5** | 分布式节点 (§14) | 后端 | 大 |
| **P6** | UI 打磨（动画、响应式、加载骨架） | 前端 | 中 |

---

## 6. 编译状态

```bash
cargo check     # 通过，10 warnings（全部有意预留）
bun run build   # 通过，0 errors，0 warnings
cargo test      # 16/16 通过
```

## 7. 关键依赖

```toml
tauri = "2" (features = ["tray-icon"])
sea-orm = "1" (sqlx-sqlite + macros + runtime-tokio)
tokio = "1" (process + sync + time + rt + macros)
axum = "0.7"
tower-http = "0.5" (cors)
pest = "2" + pest_derive
lofty = "0.21"
blake3 = "1"
walkdir = "2"
dirs = "5"
lucide-svelte = "1" (前端图标)
```
