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

### 前端 (Svelte) — daisyUI 重构

> 前端已整体重构为 **daisyUI (Tailwind v4) + lucide-svelte 图标库**，
> 视觉结构对齐 LX Music（参考 `PROMPTS/search.png` / `list.png` / `detailed.png`）。

| 功能 | 状态 | 说明 |
|------|------|------|
| **UI 框架** | ✅ | daisyUI 5 + Tailwind v4（`@tailwindcss/vite`），`dark` 主题 |
| **图标库** | ✅ | lucide-svelte |
| **无边框窗口** | ✅ | 自定义标题栏 + Windows 风格控制按钮 |
| **窗口大小持久化** | ✅ | localStorage 保存/恢复尺寸和最大化状态 |
| **左侧图标栏** | ✅ | 搜索/列表/设置 三项，active 高亮 |
| **搜索页 /search** | ✅ | 贴合 search.png：平台标签、Song/Playlist 切换、搜索历史 chips、结果列表 |
| **列表库页 /** | ✅ | 左侧「搜索预设」侧栏 + 歌曲表格(可调列宽/排序/跳转详情)；列 = #/名称(flex:1)/标签/歌手/专辑/时长(52px固定) |
| **播放详情覆盖层** | ✅ | 贴合 detailed_page.png：左侧图文块靠左；底部=左右两块(右=prev/play/next大按钮；左=上controls靠右/中进度条占满/下时间靠右)、maximize图标修复 |
| **播放栏** | ✅ | 封面 + 歌名/歌手 + 时间 + 控制键 + 上一首/播放/下一首；音量/循环弹窗复用共享组件 |
| **共享播放控制** | ✅ | `VolumeControl` + `PlayModeControl`（播放栏/详情页复用）；音量弹窗=百分比+静音checkbox+调节条；循环弹窗=3个toggler(单曲/列表、顺序/逆序、播完暂停/循环)组合成8种模式 |
| **搜索预设** | ✅ | 侧栏「+」手动添加（名称+查询串），存 SQLite，点击即应用查询 |
| **搜索历史** | ✅ | localStorage 持久化，最近 20 条 |
| **标题栏全局搜索** | ✅ | 回车 → 跳 `/search`，下拉显示本地自动补全 |
| **进度条 Seek** | ✅ | 播放详情内可点击/键盘 seek |
| **标签编辑悬浮窗** | ✅ | 按命名空间分组 + 增删 + 自动补全 |
| **右键菜单** | ✅ | 歌曲行右键：播放/编辑标签/跳转详情/清空该曲缓存/下载(占位) |
| **播放栏右键** | ✅ | 右击播放栏→在列表中定位当前歌曲 / 编辑当前歌曲标签 |
| **记住播放进度** | ✅ | 播放中轮询保存 last_played(song_id+position)；启动时可自动播放并seek恢复 |
| **记住列表滚动位置** | ✅ | 列表页 scrollTop 存 localStorage，重启/返回恢复；`?locate=` 定位并滚动到指定歌曲 |
| **歌词增强** | ✅ | 当前行放大高亮；拖拽歌词改进度；右击歌词调字号(存设置) |
| **设置页** | ✅ | 对齐 settings.png：通用/外观(窗口缩放·字体 热调整)/歌词/库管理(包含+排除文件夹完整CRUD，扫描与添加并排)/数据(4按钮并排：导出导入歌曲·设置)；无侧边栏 |
| **设置持久化** | ✅ | `AppSettings` 存 SQLite settings 表（get/set_app_settings），纯UI项(列宽等)仍存 localStorage |
| **JSON 导出** | ✅ | 全部歌曲导出为 JSON |

> 说明：原 `/playlists` 路由已并入列表页的「搜索预设」侧栏（查询语言构建动态视图）。

### 插件系统

| 组件 | 状态 | 路径 |
|------|------|------|
| PluginManager 编排 | ✅ | `features/plugin/mod.rs` |
| 路由表 (Mutex) | ✅ | `features/plugin/router.rs` |
| 运行时 + 超时 | ✅ | `features/plugin/runtime.rs` |
| 类型定义 | ✅ | `features/plugin/types.rs` |
| **yt-dlp 适配器** | ✅ 已创建 | `plugins/ytdlp-adapter/`（需安装 yt-dlp） |

---

## 3. Tauri 命令清单（共 40 个）

```rust
// Library (6)
list_songs()              → Vec<Song>            // 全量
list_library_songs()      → Vec<Song>            // 我的库（有路径/标签）
get_song(id)              → Option<Song>
search_songs(query)       → Vec<Song>
scan_library()            → ScanResult
import_songs(songs)       → u64                  // 批量upsert歌曲
clear_song_cache(id)      → u64                  // 清空歌曲缓存

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

// Search Presets (5)
list_search_presets()          → Vec<SearchPreset>
save_search_preset(name, query) → SearchPreset
delete_search_preset(id)        → ()
get_last_search_preset()        → Option<String>
set_last_search_preset(id)      → ()

// App Settings (2)
get_app_settings()             → AppSettings
set_app_settings(settings)     → ()

// Playback memory (3)
get_last_played()             → Option<(song_id, position)>
save_last_played(song_id, pos) → ()
clear_song_cache(id)           → u64

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
