# Vide 插件开发指南

Vide 的插件系统基于 **外部子进程 + JSON lines over stdin/stdout** 通信。
插件可以用**任何语言**编写——只要它能读取 stdin 和写入 stdout。

---

## 目录

1. [架构概览](#1-架构概览)
2. [插件目录结构](#2-插件目录结构)
3. [manifest.json](#3-manifestjson)
4. [IPC 协议](#4-ipc-协议)
5. [Action 参考](#5-action-参考)
6. [StreamDescriptor](#6-streamdescriptor)
7. [Capability 类型](#7-capability-类型)
8. [错误处理](#8-错误处理)
9. [完整示例：yt-dlp 适配器](#9-完整示例yt-dlp-适配器)

---

## 1. 架构概览

```
┌──────────┐    JSON lines     ┌──────────────┐
│  Vide    │ ←─── stdin/stdout ──→ │  插件进程     │
│  (Rust)  │                      │  (任何语言)    │
└──────────┘                     └──────────────┘
```

**关键规则**:
- 插件**只负责**提供结构化数据（搜索结果、元数据、流描述符）
- 核心**负责所有 I/O**：网络请求、文件写入、缓存、播放
- 插件**绝不**写入磁盘或管理文件路径
- 插件使用 **stdout** 输出 JSON，**stderr** 输出日志（核心忽略 stderr）
- 核心忽略任何非 JSON 的 stdout 输出

**黄金法则**: 插件决定获取什么。核心决定如何获取。

---

## 2. 插件目录结构

每个插件在 `plugin_dir` 下拥有一个独立子目录：

```
plugins/
├── ytdlp-adapter/
│   ├── manifest.json       # 必选：插件清单
│   ├── adapter.py           # 插件代码（任意语言）
│   └── requirements.txt     # 依赖声明（Python）
│
└── another-plugin/
    ├── manifest.json
    └── plugin.js
```

`plugin_dir` 默认值：`~/.config/vide/plugins/`（可通过设置更改）。

---

## 3. manifest.json

```json
{
  "name": "ytdlp-adapter",
  "version": "1.0.0",
  "exec": ["python3", "adapter.py"],
  "capabilities": [
    { "type": "search", "platform": "youtube" },
    { "type": "search", "platform": "bilibili" },
    { "type": "metadata", "platform": "youtube" },
    { "type": "stream", "platform": "youtube" },
    { "type": "stream", "platform": "bilibili" }
  ]
}
```

### 字段说明

| 字段 | 类型 | 必选 | 说明 |
|------|------|------|------|
| `name` | string | ✅ | 插件唯一名称，用于路由和日志 |
| `version` | string | ✅ | 语义化版本号 |
| `exec` | string[] | ✅ | 启动命令和参数，例如 `["node", "plugin.js"]` |
| `capabilities` | object[] | ✅ | 插件提供的能力列表 |

### exec 示例

| 语言 | exec 值 |
|------|---------|
| Node.js | `["node", "plugin.js"]` |
| Python | `["python3", "adapter.py"]` |
| 编译二进制 | `["./plugin-binary"]` |
| Shell | `["bash", "plugin.sh"]` |

### capabilities 条目

```json
{ "type": "search", "platform": "youtube" }
```

- `type`: 能力类型（见 §7）
- `platform`: 目标平台标识，例如 `"youtube"`, `"bilibili"`, `"spotify"`

---

## 4. IPC 协议

所有通信使用 **JSON lines** 格式：每行一个 JSON 对象，以 `\n` 分隔。

### 请求（核心 → 插件）

```json
{"id": "t001", "action": "search", "params": {"q": "hello", "platform": "youtube"}}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | string | 请求 ID（核心生成，插件必须在响应中原样返回） |
| `action` | string | 操作类型（见 §5） |
| `params` | object | 操作参数（取决于 action） |

### 响应（插件 → 核心）

**成功**:
```json
{"id": "t001", "result": [...]}
```

**错误**:
```json
{"id": "t001", "error": "not_supported"}
```

**流描述符**（stream action 的响应）:
```json
{"id": "t001", "type": "url_stream", "url": "https://...", "headers": {"Referer": "..."}}
```

| 字段 | 类型 | 必选 | 说明 |
|------|------|------|------|
| `id` | string | ✅ | 对应请求的 ID |
| `result` | any | 二选一 | 成功的结果数据 |
| `error` | string | 二选一 | 错误消息 |

### 协议规则

1. 每行一个完整的 JSON 对象，以 `\n` 结尾
2. 核心发送请求后，等待一行 JSON 响应
3. 非 JSON 行被核心静默忽略
4. 插件使用 stderr 输出日志，核心完全忽略 stderr
5. 支持超时：search 10s, stream 30s, resolve 5s
6. 超时后核心发送 SIGTERM → 等待 → SIGKILL
7. 插件以非零状态退出 → 视为失败，核心触发回退

---

## 5. Action 参考

### `search`

在平台上搜索歌曲。

**params**:
```json
{
  "q": "搜索关键词",
  "platform": "youtube"
}
```

**result**（数组）:
```json
[
  {
    "platform": "youtube",
    "id": "abc123",
    "title": "歌曲名称",
    "artist": "艺术家",
    "album": "专辑名",
    "duration": 240.5
  }
]
```

| 字段 | 类型 | 必选 | 说明 |
|------|------|------|------|
| `platform` | string | ✅ | 平台标识 |
| `id` | string | ✅ | 平台上的唯一 ID（和 platform 一起构成 song_id = hash(platform:id)） |
| `title` | string | ✅ | 歌曲标题 |
| `artist` | string | 可选 | 艺术家 |
| `album` | string | 可选 | 专辑 |
| `duration` | number | 可选 | 时长（秒） |

### `resolve`

获取已知歌曲的完整元数据。

**params**:
```json
{
  "platform": "youtube",
  "id": "abc123"
}
```

**result**: 同 search 的单条结果。

### `stream`

获取可播放的流描述符。

**params**:
```json
{
  "platform": "youtube",
  "id": "abc123",
  "song_id": "hexhash..."
}
```

**result**: StreamDescriptor（见 §6）。

---

## 6. StreamDescriptor

插件返回的流描述符只有两种有效类型：

### 类型 A：直接 URL 流

```json
{
  "id": "t001",
  "type": "url_stream",
  "url": "https://cdn.example.com/media.mp3",
  "headers": {
    "Referer": "https://example.com",
    "User-Agent": "Mozilla/5.0"
  }
}
```

- 核心直接通过 HTTP 获取 URL
- `headers` 可选，用于需要 Referer/Cookie 的平台

### 类型 B：子进程管道

```json
{
  "id": "t001",
  "type": "stream",
  "exec": {
    "cmd": ["yt-dlp", "-o", "-", "https://youtube.com/watch?v=abc123"],
    "mode": "stdout"
  }
}
```

- 核心启动 `cmd` 子进程
- 管道的 stdout 输出被传入 mpv 播放
- 适用于需要实时转码/解密的场景（如 yt-dlp）

### 优先策略

插件应**优先返回 `url_stream`**（直接 URL 流效率最高）。
仅在无法获取直接 URL 时才回退到 `pipe` 模式。

---

## 7. Capability 类型

| 能力 | 说明 |
|------|------|
| `search` | 搜索歌曲 |
| `metadata` | 获取歌曲元数据 |
| `playback` | 返回流描述符（等同于 `stream`） |
| `lyrics` | 获取歌词 |
| `recommendation` | 推荐相关歌曲（预留） |

系统设计为可扩展——未来可以增加新的能力类型而无需修改核心。

---

## 8. 错误处理

### 错误响应格式

```json
{"id": "t001", "error": "not_supported"}
```

### 标准错误码

| error | 说明 |
|-------|------|
| `not_supported` | 插件不支持该 action |
| `not_found` | 未找到请求的资源 |
| `timeout` | 上游服务超时 |
| `rate_limited` | 被平台限流 |
| `auth_required` | 需要用户认证/登录 |
| `internal_error` | 插件内部错误 |

### 回退机制

1. 插件返回 `error` → 核心记录错误
2. 如果该路由配置了多个 provider → 核心自动尝试下一个
3. 所有 provider 都失败 → 返回错误给用户

---

## 9. 完整示例：yt-dlp 适配器

这是一个 Python 插件的完整实现，使用 yt-dlp 搜索和播放 YouTube/Bilibili 内容。

### 目录结构

```
plugins/ytdlp-adapter/
├── manifest.json
├── adapter.py
└── requirements.txt
```

### manifest.json

```json
{
  "name": "ytdlp-adapter",
  "version": "1.0.0",
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

### requirements.txt

```
yt-dlp>=2024.0.0
```

### adapter.py

```python
#!/usr/bin/env python3
"""Vide 插件适配器：通过 yt-dlp 搜索和播放在线媒体。"""

import json
import sys
import subprocess
import urllib.parse

YTDLP = ["yt-dlp", "--no-warnings", "--ignore-errors"]


def handle_search(params):
    """搜索平台内容。"""
    q = params.get("q", "")
    platform = params.get("platform", "youtube")

    # yt-dlp 搜索格式: ytsearchN:query
    search_query = f"ytsearch10:{q}" if platform == "youtube" else q

    cmd = YTDLP + [
        "--flat-playlist",
        "-j",
        search_query,
    ]

    try:
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=15)
        items = []
        for line in result.stdout.strip().split("\n"):
            if not line:
                continue
            try:
                data = json.loads(line)
                items.append({
                    "platform": platform,
                    "id": data.get("id", ""),
                    "title": data.get("title", "Unknown"),
                    "artist": data.get("channel", data.get("uploader", "")),
                    "duration": data.get("duration"),
                })
            except json.JSONDecodeError:
                continue
        return items
    except subprocess.TimeoutExpired:
        return {"error": "timeout"}
    except Exception as e:
        return {"error": str(e)}


def handle_resolve(params):
    """解析单一项目的元数据。"""
    platform = params.get("platform", "youtube")
    item_id = params.get("id", "")

    url = f"https://www.youtube.com/watch?v={item_id}" if platform == "youtube" \
          else f"https://www.bilibili.com/video/{item_id}"

    cmd = YTDLP + ["-j", url]

    try:
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=20)
        data = json.loads(result.stdout.strip())
        return {
            "platform": platform,
            "id": data.get("id", item_id),
            "title": data.get("title", "Unknown"),
            "artist": data.get("channel", data.get("uploader", "")),
            "duration": data.get("duration"),
        }
    except Exception as e:
        return {"error": str(e)}


def handle_stream(params):
    """返回流描述符。"""
    platform = params.get("platform", "youtube")
    item_id = params.get("id", "")

    url = f"https://www.youtube.com/watch?v={item_id}" if platform == "youtube" \
          else f"https://www.bilibili.com/video/{item_id}"

    # 尝试先获取直接 URL（最优先）
    cmd = YTDLP + [
        "-g",  # 获取直接 URL
        "-f", "bestaudio/best",
        url,
    ]

    try:
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=20)
        direct_url = result.stdout.strip()
        if direct_url and direct_url.startswith("http"):
            return {
                "type": "url_stream",
                "url": direct_url,
                "headers": {
                    "User-Agent": "Mozilla/5.0",
                },
            }
    except Exception:
        pass

    # 回退到 pipe 模式
    return {
        "type": "stream",
        "cmd": YTDLP + ["-o", "-", "-f", "bestaudio/best", url],
        "mode": "stdout",
    }


def main():
    """主循环：读取 stdin 的 JSON lines，处理请求，写入 stdout。"""
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue

        try:
            request = json.loads(line)
        except json.JSONDecodeError:
            continue

        req_id = request.get("id", "unknown")
        action = request.get("action", "")
        params = request.get("params", {})

        # 分发 action
        if action == "search":
            result = handle_search(params)
        elif action == "resolve":
            result = handle_resolve(params)
        elif action == "stream":
            result = handle_stream(params)
        else:
            response = {"id": req_id, "error": "not_supported"}
            print(json.dumps(response), flush=True)
            continue

        # 检查是否返回了错误
        if isinstance(result, dict) and "error" in result:
            response = {"id": req_id, "error": result["error"]}
        elif action == "stream":
            # stream 返回 StreamDescriptor
            result["id"] = req_id
            response = result
        else:
            response = {"id": req_id, "result": result}

        print(json.dumps(response), flush=True)


if __name__ == "__main__":
    main()
```

---

## 附录：Rust 核心侧相关代码索引

| 功能 | 文件 |
|------|------|
| IPC 协议类型 | `src-tauri/src/infra/ipc.rs` |
| 插件清单类型 | `src-tauri/src/features/plugin/types.rs` |
| 清单加载 | `src-tauri/src/features/plugin/manifest.rs` |
| 路由表 | `src-tauri/src/features/plugin/router.rs` |
| 进程运行时 | `src-tauri/src/features/plugin/runtime.rs` |
| 插件系统编排 | `src-tauri/src/features/plugin/mod.rs` |
| 流描述符类型 | 同在 `types.rs` |
| 超时常量 | `runtime.rs`: `TIMEOUT_SEARCH`=10s, `TIMEOUT_STREAM`=30s, `TIMEOUT_RESOLVE`=5s |
