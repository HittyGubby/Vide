#!/usr/bin/env python3
"""Vide 插件适配器：通过 yt-dlp 搜索和播放在线媒体。"""

import json
import subprocess
import sys
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
                items.append(
                    {
                        "platform": platform,
                        "id": data.get("id", ""),
                        "title": data.get("title", "Unknown"),
                        "artist": data.get("channel", data.get("uploader", "")),
                        "duration": data.get("duration"),
                    }
                )
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

    url = (
        f"https://www.youtube.com/watch?v={item_id}"
        if platform == "youtube"
        else f"https://www.bilibili.com/video/{item_id}"
    )

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

    url = (
        f"https://www.youtube.com/watch?v={item_id}"
        if platform == "youtube"
        else f"https://www.bilibili.com/video/{item_id}"
    )

    # 尝试先获取直接 URL（最优先）
    cmd = YTDLP + [
        "-g",  # 获取直接 URL
        "-f",
        "ba",
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

    # 回退到 pipe 模式 (用 bestaudio/best 更稳健)
    return {
        "type": "stream",
        "cmd": YTDLP + ["-o", "-", "-f", "bestaudio/best", url],
        "mode": "stdout",
    }


def handle_playlist(params):
    """解析一个在线播放列表 URL，返回其中的条目列表。

    params: { "url": "https://www.youtube.com/playlist?list=..." }
    """
    url = params.get("url", "")
    if not url:
        return {"error": "no url"}

    # 用 yt-dlp 扁平化列出播放列表条目 (不拉取每个条目的完整元数据)
    cmd = YTDLP + ["--flat-playlist", "-J", url]
    try:
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
        data = json.loads(result.stdout.strip())
    except subprocess.TimeoutExpired:
        return {"error": "timeout"}
    except Exception as e:
        return {"error": str(e)}

    entries = data.get("entries", []) if isinstance(data, dict) else []
    items = []
    for ent in entries:
        if not isinstance(ent, dict):
            continue
        eid = ent.get("id", "")
        if not eid:
            continue
        items.append({
            "platform": ent.get("extractor_key", "").lower(),
            "id": eid,
            "title": ent.get("title", "Unknown"),
            "artist": ent.get("channel", ent.get("uploader", "")),
            "duration": ent.get("duration"),
        })
    return items


def handle_lyrics(params):
    """获取歌词（当前仓库内暂无独立歌词源；返回空列表表示无歌词）。"""
    return []


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
        elif action == "playlist":
            result = handle_playlist(params)
        elif action == "lyrics":
            result = handle_lyrics(params)
        else:
            response = {"id": req_id, "error": "not_supported"}
            print(json.dumps(response), flush=True)
            continue

        # 所有响应统一包裹为 {"id": ..., "result": ...} 或 {"id": ..., "error": ...}
        if isinstance(result, dict) and "error" in result:
            response = {"id": req_id, "error": result["error"]}
        else:
            response = {"id": req_id, "result": result}

        print(json.dumps(response), flush=True)


if __name__ == "__main__":
    main()
