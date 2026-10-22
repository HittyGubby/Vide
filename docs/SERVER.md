# Vide Server

Vide Server 是 Rust/axum + SQLite 的服务端，同时提供：

- 权威音乐库 API
- 媒体流 / 下载
- 认证与会话
- 服务端播放器
- 静态 Web 客户端
- SSE 播放器状态
- 插件搜索/解析/缓存
- 审计日志

## 构建

```bash
bun install
bun run build

cargo build --release \
  --manifest-path src-tauri/Cargo.toml \
  --bin vide-server
```

二进制：

```text
src-tauri/target/release/vide-server
```

## 首次设置密码

```bash
./src-tauri/target/release/vide-server \
  --data-dir ~/.local/share/vide \
  --set-password 'your-password'
```

密码使用 Argon2id hash 保存到 `vide.db`。

## 启动

```bash
./src-tauri/target/release/vide-server \
  --data-dir ~/.local/share/vide \
  --bind 0.0.0.0:8080 \
  --web-dir "$PWD/build"
```

参数：

| 参数 | 说明 |
| --- | --- |
| `--data-dir` | SQLite、缓存、审计日志目录；默认系统数据目录下的 vide |
| `--bind` | 默认 `[::]:8080` |
| `--web-dir` | Svelte build 目录，默认仓库 build |
| `--public-media` | 游客可读取音频字节 |
| `--tls-cert` / `--tls-key` | 同时提供时启用 HTTPS |
| `--set-password` | 设置密码后退出 |

## TLS

```bash
vide-server \
  --data-dir /var/lib/vide \
  --bind 0.0.0.0:8080 \
  --web-dir /opt/vide/build \
  --tls-cert /path/cert.pem \
  --tls-key /path/key.pem
```

启用 TLS 后登录 Cookie 带 `Secure`。

公网部署必须使用可信证书；自签证书需要客户端信任。

## systemd 用户服务

推荐用当前用户运行，避免服务端生成的数据库/缓存变成 root 所有。

安装：

```bash
sudo mkdir -p /opt/vide
sudo cp vide-server /opt/vide/vide-server
sudo cp -a build /opt/vide/web
mkdir -p ~/.config/systemd/user
cp deploy/vide.service ~/.config/systemd/user/vide.service
systemctl --user daemon-reload
systemctl --user enable --now vide.service
loginctl enable-linger "$USER"
```

服务文件见 [deploy/vide.service](../deploy/vide.service)。

查看日志：

```bash
journalctl --user -u vide.service -f
```

## 数据目录

```text
vide.db           SQLite
cache/            <song_id>.media 音频缓存
audit.log         审计日志
```

## 认证

- 浏览器：Cookie `vide_session` + CSRF
- 非浏览器/桌面远程：Bearer token + CSRF
- 登录限流：60 秒最多 5 次失败
- Session 持久化在 SQLite，服务端重启后仍有效
- 游客可读公开接口
- 写操作、服务端播放、下载需要登录

桌面游客远程可以不登录连接，只能读。

## 主要 API

### 公开只读

```text
GET  /api/v1/status
GET  /api/v1/songs
GET  /api/v1/songs/:id
GET  /api/v1/search?q=
GET  /api/v1/search/online
GET  /api/v1/plugins/state       # 游客脱敏
GET  /api/v1/player/status
GET  /api/v1/player/stream       # SSE
GET  /api/v1/player/queue
GET  /api/v1/media/:id           # Range
```

### 登录 + CSRF

```text
POST /api/v1/auth/login
POST /api/v1/auth/logout
GET  /api/v1/auth/me
POST /api/v1/player/play
POST /api/v1/player/play-online
POST /api/v1/player/queue
POST /api/v1/player/stop
POST /api/v1/player/next
POST /api/v1/player/prev
POST /api/v1/player/toggle
POST /api/v1/player/seek
POST /api/v1/player/volume
POST /api/v1/player/mute
POST /api/v1/player/speed
GET  /api/v1/lyrics/:id
POST /api/v1/tags
POST /api/v1/tags/remove
POST /api/v1/songs/:id/metadata
GET  /api/v1/songs/:id/tags
GET  /api/v1/namespaces
GET  /api/v1/meta-tags
GET  /api/v1/tag-values
POST /api/v1/import/songs
GET  /api/v1/server-settings
POST /api/v1/server-settings
POST /api/v1/library/scan
GET  /api/v1/cache/stats
POST /api/v1/cache/clear
GET  /api/v1/download/:id
POST /api/v1/resolve
POST /api/v1/plugins/reload
POST /api/v1/plugins/enabled
POST /api/v1/plugins/argv
POST /api/v1/plugins/platform-argv
POST /api/v1/plugins/route-argv
POST /api/v1/plugins/concurrency
POST /api/v1/plugins/route
```

## SSE

```text
GET /api/v1/player/stream
```

事件：

```text
event: player_status
data: {"state":"Playing","media_id":"...","elapsed_secs":12.3,"duration_secs":203.1}
```

服务端每秒刷新并广播一次，所有客户端共享同一条 SSE 流，避免每客户端每秒 HTTP 轮询。

## 下载

```text
GET /api/v1/download/:id
```

- 需要登录
- 本地文件、缓存或在线歌曲均可
- 返回 `Content-Disposition: attachment`
- 缓存文件扩展名 `.media` 默认导出为 `.mp3`

## 缓存

`GET /api/v1/cache/stats` 返回图片/音频/歌词/元信息统计。

`POST /api/v1/cache/clear`：

```json
{ "kind": "audio" }
```

支持 `audio`、`lyric`、`metadata`。

## 审计

写入：

```text
<data-dir>/audit.log
```

包括登录、播放、扫描、标签、导入、缓存清理。

## 安全

- 公开读、登录写
- 服务端路径不出现在公开歌曲响应
- 登录限流
- 可选 TLS
- CORS allowlist 支持 Tauri origin
- 公网部署不要使用默认密码/HTTP

## 限制

- 单用户/单信任域，没有多用户租户
- 服务端播放依赖服务器安装 mpv
- 在线歌曲首次播放/下载依赖插件和网络
- 密码重置只通过 CLI `--set-password`
