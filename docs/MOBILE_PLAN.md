# Vide 移动端 Web 视图与模块化计划

> 状态：P0 ✅、P1 ✅（曲库）、P2 ✅（搜索）、P3/P3b ✅（设置与拆分）、P4 ✅（`/player`）、P5 ✅（性能）。可选收尾见 §10。
> 按 P0 → P5 顺序推进。
> 范围：手机浏览器的响应式 Web 视图（含 PWA 之外的移动体验）；**不做** Tauri 移动端、**不做** PWA/Service Worker/离线/IndexedDB。

## 1. 断点与判定

手机与桌面**只有两档**，平板走桌面 UI：

```
mobileUI = min(innerWidth, innerHeight) < 600
```

- 短边判定：手机横屏（852×393）仍是移动 UI，iPad（768×1024）是桌面 UI。
- **不使用 UA 判定**：iPadOS 报 Mac UA、折叠屏、桌面版网站模式都会判错；宽度判定自洽。
- **不使用视图缩放**：缩放会让文字糊、和系统字号打架；可访问性交给布局。
- `orientation` 与 `(pointer: coarse)` 只做样式/交互微调，不参与"换整块组件"。
- 单点实现：`src/lib/platform.svelte.ts`。组件内不用 `{#if}` 猜宽度，只有换组件时读 `platform.mobileUI`。

## 2. 壳结构

```
+layout.svelte
├── 歌词窗（Tauri，label=lyrics）      → 裸渲染 children
├── Tauri 桌面                          → DesktopShell（现有外观）
└── Web
    ├── platform.mobileUI = false       → WebShell（现有外观 + 登录/播放位置）
    └── platform.mobileUI = true        → MobileShell
                                          ├── Header（☰ + 标题 / 查询栏 / 在线搜索栏）
                                          ├── Drawer（列表 · 搜索 · 设置 · 登录 · 播放位置）
                                          ├── content
                                          └── PlayerBar（P1 换成 MiniPlayer）
```

桌面壳专有的运行时（窗口几何、全局快捷键、桌面歌词窗、缩放、设置加载、上次播放恢复）全部留在 `DesktopShell.svelte`，不再污染根布局。Web 端在根布局统一加载 settings / auth / server status。

## 3. 页面映射

| 桌面 | 移动 |
|---|---|
| 曲库表格（列宽/列序/显隐） | 单列列表：序号/播放中 + 标题 + 第二行 `歌手 · 专辑 · 标签`（纯文本，无胶囊） + 时长 + ⋮ |
| 表头右键 = 列显隐 | 无（隐藏列设置），排序进排序面板 |
| hover 悬浮卡 | 长按 / ⋮ → 底部动作面板 |
| 右键菜单 | 长按 / ⋮ → 底部动作面板（同一份 MenuItem 数据） |
| Ctrl/Shift 多选 | 长按进入选择模式，顶栏换成 全选/取消/已选 N |
| 顶部查询栏 | 同 PC：输入框 + 查询面板 + 自动补全；面板全宽、高度上限 50dvh |
| 搜索页：页码 | 加载更多（客户端累计，配合虚拟列表） |
| 设置：单页分组 | 单页 + `?section=` 两级（移动端先看分类列表，进入后只看该分类） |
| 播放详情 overlay | `/player` 路由（桌面也改），移动端封面/歌词滚动分页 |
| 桌面歌词 / 快捷键 / 窗口缩放 | 移动端隐藏（能力位控制） |

## 4. 交互替换

| 桌面 | 移动 |
|---|---|
| hover 预览 | 长按 → 详情面板 |
| 右键 | ⋮ / 长按 → bottom sheet |
| 列拖拽 | 无 |
| 键盘快捷键 | 无 |
| 音量/播放模式 hover popup | 点击 → sheet（`@media (hover: hover)` 分流） |
| 下载（登录） | 下载（登录 或 `public_media` 为真） |

触控细节：`100dvh` + `env(safe-area-inset-*)`、`viewport-fit=cover`、`touch-action: manipulation`、`-webkit-touch-callout: none`、长按 `preventDefault`、触控目标 ≥ 44px、`visualViewport` 避让软键盘、`MediaSession`。

## 5. 模块化

**原则：共享"单元格 / 动作 / 状态"，不共享"整行布局"。**
历史教训：`76479c6` 抽出过共享 SongRow，`9614b43` 因两边行样式分叉又删掉。改为共享 cell + 各自行布局。

目标结构（P1 完成后）：

```
src/lib/
  platform.svelte.ts            # tier / orientation / coarse / 能力位
  shell/                        # AppShell + DesktopShell + WebShell + MobileShell
  ui/                           # Dialog / MenuList / MenuSheet
  library/
    columns.svelte.ts           # 列定义 + 持久化 + 调宽/调序/显隐/fit
    selection.svelte.ts         # 锚点 / 多选 / 全选
    actions.ts                  # play / download / cache / details / bulk
    virtual.svelte.ts           # 行高 + 虚拟窗口
    DesktopLibraryTable.svelte
    MobileSongList.svelte
  song/
    cells/*.svelte              # TitleCell / TagCell / ArtistCell / DurationCell / IndexCell
    ActionSheet.svelte
  settings/sections/*.svelte
```

## 6. 性能清单

1. **删除 Monaco**（构建产物 13MB：主 chunk 4MB / ts.worker 7MB / css·html·json worker）→ 设置里的 CSS 编辑器改用 CodeMirror（新增 `@codemirror/lang-css`）。
2. **修 131 处无效 CSS**：`color: var(--x) / 0.6` 不是合法 CSS，浏览器丢弃声明 → 统一 `color-mix(in oklch, var(--x) 60%, transparent)`。颜色仍走 CSS 变量（后续深/浅色主题与自定义主题色都依赖它）。
3. **滚动位置**：Web/远程的 `set_client_state` 本来就由 transport 落到 localStorage，没有网络写入；本地 Tauri 才写 SQLite。保留 300ms 防抖即可。`measureScrollbar` 仍在 scroll 事件里做布局读取，后续可移到 ResizeObserver。
4. **列表 payload 瘦身**：`public_song` 目前保留 `metadata.lyrics`/`notes`，列表响应按全库返回；列表 DTO 剔除重字段，`get_song` 保留。
5. **服务端压缩 + 缓存头**：`tower-http` 加 `CompressionLayer`；`/_app/immutable/*` 加 `Cache-Control: immutable`。
6. **虚拟化**：`@tanstack/svelte-virtual`（`scrollToIndex` 取代现在 40×50ms 轮询），搜索"加载更多"配合使用。
7. **列渲染**：已抽 `columnLayout.svelte.ts` 并由它驱动桌面表的 flex 列（宽度就是列宽），TanStack Table 已移除；搜索页共用同一套。
8. **封面**：线上封面已由 `/api/v1/image` 在服务端落盘缓存（`cache/images`，浏览器缓存之外的第二层）；本地歌曲内嵌封面仍待做——Web 走 `/api/v1/cover/:id`，Tauri 走缓存文件 + asset protocol，无封面回落大 V。

## 7. 数据修复

### 7.1 added_time

- 存储格式：`YYYY/MM/DD HH:MM:SS`（斜杠，精确到秒，本地时区）。
- `songs.added_time` **列是唯一真相**，读取时列优先，`metadata_json` 里那份只做兼容。
- 来源：扫描/本地导入 = 文件 mtime；在线导入/来源歌曲 = 导入时间。
- `upsert` 冲突时不再用空值覆盖；去掉启动时的 `backfill_added_time`（一次性迁移不进代码）。
- 存量修复由最后一次"新开对话"完成，规格：
  - 旧值形如 `2026-10-26`，目标是 `2026/10/26 HH:MM:SS`；
  - 需要文件时间的，要按 `song_path` 表逐个文件取 mtime（纯 SQL 做不到，需要脚本/命令）；
  - 只改 `songs.added_time` 一列即可（JSON 那份不参与读取）；
  - 过渡期副作用：新旧格式混排时 `ORDER BY added_time DESC` 会把新格式整体排在前面（新歌仍在最前），等值/上界查询会失配，修复后统一。
- 显示：桌面悬浮卡、移动端详情面板。
- **已执行（2026-10-26）**：`scripts/migrate-added-time.mjs` 迁移本机库 4298 行（1799 行用文件 mtime，2499 行用当日 00:00:00）；备份 `vide.db.bak-2026-10-26T14-09-08-985Z`。

### 7.2 tag 历史

- 排序：`last_used` 改毫秒级时间戳，保证"最近添加的在最前"。
- 删除：新增 `remove_tag_history(namespace, value)`（Tauri command + `POST /api/v1/tag-history/remove` + transport 映射）；游客/Web 同步 localStorage 镜像。
- UI：历史区右上角「删除历史」→ 变「完成」，此时胶囊右上角出现 ✕；删除模式下胶囊本体失效变灰；立即生效、不可撤销；桌面/移动同一套。

## 8. 阶段

| 阶段 | 内容 | 验收 |
|---|---|---|
| P0 ✅ | 文档、platform 层、壳拆分、弹层原语、CSS 修复、列布局共享模块、added_time/tag 历史、清理。`selection / actions / virtual` 随 P1 一起抽 | 桌面外观不变、`bun run check` 通过 |
| P1 ✅ | 曲库纵向切片：数据 store、共享列/选择/虚拟/动作模块、移动列表、MiniPlayer、排序/操作 sheet、移除 TanStack、桌面紧凑化（34px 行高 / 30px 表头 / 6px 内边距） | 手机可用完整曲库 |
| P2 ✅ | 搜索切片：通用 `VirtualMobileList`（曲库与搜索结果共用滚动/虚拟/手势）、移动搜索结果行（封面 · 平台/歌手/专辑/标签）、加载更多、选择态与底部动作面板；封面懒加载 + bilibili 直接走代理；服务端图片磁盘缓存（url+referer 为键，`.type` sidecar，缓存统计不再计 sidecar） | 手机可搜索、加载更多、播放与编辑 |
| P3 ✅ | 设置切片：单路由 + 移动端分类列表 / `?section=` 两级、桌面 sticky 分区导航 + 滚动高亮；10 个分区组件 + 共享 `SettingsGroup/SettingCard/SettingRow/SettingToggle`；PluginSettings 与 RemoteConnection 改为纯内容组件；表单从 table 改为响应式行式布局 | 手机可逐项配置 |
| P3b ✅ | PluginSettings 拆成 `PluginCard`（单个插件+详情）、`RouteSettings`（平台路由+argv）与 `pluginHelpers`；TagEditor 拆成 `TagSelected` / `TagHistory` / `TagCatalog` / `TagInput` | 文件行数下降、行为不变 |
| P4 ✅ | `/player` 成为正式路由（桌面与手机共用）：去掉 `ui.showPlayerDetail` 覆盖层，播放页不再渲染壳；桌面沿用原 PlayerDetail（含 Tauri 窗口控制与拖拽区），手机用新的 `MobilePlayerDetail`（封面/歌词 scroll-snap 分页、进度、传输键、播放设置底部弹层：模式/音量/倍速）；歌词抽成共享 store；播放栏标题区可点击进入 | 手机播放页可用 |
| P5 ✅ | Monaco → CodeMirror（按需加载，immutable 包共 1.3 MB，最大 chunk 308 KB，无 worker）；服务端 gzip/br（带 Range 的 media/download、image 与 SSE 明确跳过）；静态哈希资源 `immutable`、HTML `no-cache`、API `no-store`（image 自己的缓存头保留）；列表 DTO 去掉歌词，导出改走 `/api/v1/songs/export`；服务端图片/音频缓存跟随 `--data-dir`；滚动条宽度移出 scroll 处理；虚拟列表沿用自研 `virtualSlice` | gzip + 强缓存生效、列表体积下降 |

## 9. 已决事项

- 移动端只做 Web，Tauri 保持桌面。
- 平板走桌面 UI；抽屉在左侧（登录、播放位置都在里面）。
- 设置保持单页；移动端用 `?section=` 两级。
- 下载对游客开放，权限跟随 `public_media`；编辑入口仍只对登录用户开放。
- 外观：深浅色模式 + 主题色。游客（Web 未登录）存 localStorage（`vide.guest-theme`，`app.html` 内联脚本在首屏前应用）；本地/登录照常写 SQLite。
- 封面：远端封面走服务端代理；本地文件的内嵌图尚未做（见 §10），当前退回大 V。
- 不做 PWA/SW/离线/IDB；不补 manifest。
- 依赖变化：删除 `monaco-editor`、`@tanstack/svelte-table`；新增 `codemirror`、`@codemirror/lang-css`、`@codemirror/theme-one-dark`。虚拟列表保留自研 `virtualSlice`，未引入 `@tanstack/svelte-virtual`。

## 10. 可选收尾（不在 P0–P5 内）

- 本地文件内嵌封面：`/api/v1/cover/:id`（服务端读 lofty picture）+ Tauri 命令/data URL；播放页与悬停卡使用。
- 用 Lighthouse 复核移动端首屏；如需要再考虑 manifest（当前明确不做 PWA）。
- 歌词首行的“居中”位置：短歌词时 `.lyrics-lines` 没有上下留白，active 行会停在顶部；如需要可加动态 padding。
