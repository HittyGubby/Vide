# Vide

Vide 是一个本地优先的音乐库与播放器，使用 Tauri 2 + Svelte 5 + Rust + SQLite。

同一套 Svelte 页面可以运行在三种形态：

- Tauri 桌面客户端，本地库
- Tauri 桌面客户端，连接远程 server
- 浏览器 Web 客户端，由 server 同源提供

## 功能

- 本地音乐扫描、标签、元数据、封面
- 本地查询语法与在线平台搜索
- 服务端权威音乐库，多设备共享
- 服务端播放与客户端播放
- 游客只读，登录后写入
- Web/远程自动恢复登录状态
- SSE 推送播放器状态，避免每个客户端轮询
- 在线歌曲服务端插件解析与缓存
- 可选 TLS、登录限流、会话持久化、审计日志

## 依赖

- Bun
- Rust stable
- Tauri Linux 依赖
- 服务端播放需要 `mpv`
- 跳过静音/音量均衡分析需要 `ffmpeg`（缺失时自动跳过分析，不影响播放）
- 插件按需提供 `python` / `yt-dlp` 等

## 本地桌面

```bash
bun install
bun run tauri dev
```

本地模式直接使用本机 SQLite，不经过 HTTP，不需要登录。

## 服务端

```bash
bun run build

cargo build --release \
  --manifest-path src-tauri/Cargo.toml \
  --bin vide-server

./src-tauri/target/release/vide-server \
  --data-dir ~/.local/share/vide \
  --set-password 'your-password'

./src-tauri/target/release/vide-server \
  --data-dir ~/.local/share/vide \
  --bind 0.0.0.0:8080 \
  --web-dir "$PWD/build"
```

默认 data-dir 是系统数据目录下的 `vide`。

生产/公网部署：

```bash
vide-server \
  --data-dir /var/lib/vide \
  --bind 0.0.0.0:8080 \
  --web-dir /opt/vide/build \
  --tls-cert /path/cert.pem \
  --tls-key /path/key.pem
```

## 客户端入口

- 桌面本地：直接启动应用
- 桌面远程：设置 → 远程连接，填 URL；密码留空为游客
- Web：打开 `http://server:8080` 或 `https://server:8080`
- 游客只读；登录后可播放控制、编辑、导入、管理库/缓存/插件

## 文档

- [文档索引](docs/README.md)
- [Server 使用](docs/SERVER.md)
- [客户端使用](docs/CLIENT.md)
- [插件协议](docs/PLUGIN_PROTOCOL.md)

## 构建 / 发布

- 前端：`bun run build`
- server：`cargo build --release --manifest-path src-tauri/Cargo.toml --bin vide-server`
- Tauri：`bun run tauri build`
- GitHub Actions：推送 `v*` tag 触发服务端和 Tauri 客户端 release

## 说明

- 公开接口只读；写操作与服务端播放需要登录
- `--public-media` 决定游客能否读取音频字节
- 默认不启用 TLS；公网部署请配置证书
- 浏览器 Web 播放使用 `<audio>`；Tauri 客户端播放使用本地 `mpv`
