#!/usr/bin/env python3
"""Vide yt-dlp adapter: YouTube and Bilibili."""

import html
import http.cookiejar
import json
import os
import re
import subprocess
import sys
import tempfile
import urllib.parse
import urllib.request

UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/120 Safari/537.36"
EXTRA = sys.argv[1:]
YTDLP = ["yt-dlp", "--no-warnings", "--ignore-errors"] + EXTRA


def bili_opener():
    jar = http.cookiejar.CookieJar()
    opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(jar))
    try:
        req = urllib.request.Request(
            "https://www.bilibili.com/",
            headers={"User-Agent": UA},
        )
        opener.open(req, timeout=10).read()
    except Exception:
        pass
    return opener


def bili_json(url):
    opener = bili_opener()
    req = urllib.request.Request(
        url,
        headers={
            "User-Agent": UA,
            "Referer": "https://www.bilibili.com/",
            "Accept": "application/json, text/plain, */*",
        },
    )
    with opener.open(req, timeout=20) as response:
        return json.loads(response.read().decode("utf-8", "replace"))


def clean_html(value):
    return html.unescape(re.sub(r"<[^>]+>", "", value or "")).strip()


def parse_duration(value):
    if isinstance(value, (int, float)):
        return float(value)
    if not isinstance(value, str) or not value:
        return None
    parts = value.split(":")
    try:
        total = 0.0
        for part in parts:
            total = total * 60 + float(part)
        return total
    except ValueError:
        return None


def bilibili_search(query, limit):
    # The API caps one page at 50; walk pages until the requested count is met.
    items = []
    seen = set()
    page = 1
    page_size = min(max(limit, 1), 50)
    while len(items) < limit and page <= 10:
        url = (
            "https://api.bilibili.com/x/web-interface/search/type?search_type=video&keyword="
            + urllib.parse.quote(query)
            + "&page="
            + str(page)
            + "&page_size="
            + str(page_size)
        )
        try:
            body = bili_json(url)
        except Exception as error:
            if items:
                break
            return {"error": "bilibili search: " + str(error)}
        if body.get("code") != 0:
            if items:
                break
            return {"error": "bilibili search: " + str(body.get("message"))}
        entries = (body.get("data") or {}).get("result") or []
        for entry in entries:
            if not isinstance(entry, dict):
                continue
            bvid = entry.get("bvid")
            if not bvid or bvid in seen:
                continue
            seen.add(bvid)
            pic = entry.get("pic") or ""
            if pic.startswith("//"):
                pic = "https:" + pic
            items.append(
                {
                    "platform": "bilibili",
                    "source_id": bvid,
                    "title": clean_html(entry.get("title")),
                    "artist": entry.get("author") or "",
                    "duration": parse_duration(entry.get("duration")),
                    "cover_url": pic or None,
                }
            )
            if len(items) >= limit:
                break
        if len(entries) < page_size:
            break
        page += 1
    return items[:limit]


def bilibili_resolve(bvid):
    body = bili_json("https://api.bilibili.com/x/web-interface/view?bvid=" + urllib.parse.quote(bvid))
    if body.get("code") != 0:
        raise RuntimeError(str(body.get("message") or "bilibili resolve failed"))
    data = body.get("data") or {}
    pic = data.get("pic") or ""
    if pic.startswith("//"):
        pic = "https:" + pic
    owner = data.get("owner") or {}
    return {
        "platform": "bilibili",
        "source_id": data.get("bvid") or bvid,
        "title": data.get("title") or "Unknown",
        "artist": owner.get("name") or "",
        "duration": data.get("duration"),
        "cover_url": pic or None,
    }


def track_from_ytdlp(platform, data):
    if not isinstance(data, dict):
        return None
    source_id = data.get("id")
    if platform == "soundcloud":
        source_id = data.get("webpage_url") or data.get("url") or source_id
    if not source_id:
        return None
    thumbnail = data.get("thumbnail")
    if not thumbnail:
        thumbnails = data.get("thumbnails") or []
        if thumbnails and isinstance(thumbnails[-1], dict):
            thumbnail = thumbnails[-1].get("url")
    return {
        "platform": platform,
        "source_id": source_id,
        "title": data.get("title") or "Unknown",
        "artist": data.get("channel") or data.get("uploader") or "",
        "duration": data.get("duration"),
        "cover_url": thumbnail,
    }


def platform_for_url(url):
    host = urllib.parse.urlparse(url).hostname or ""
    host = host.lower()
    if host.endswith("youtube.com") or host.endswith("youtu.be"):
        return "youtube"
    if host.endswith("bilibili.com") or host.endswith("b23.tv"):
        return "bilibili"
    if host.endswith("soundcloud.com"):
        return "soundcloud"
    return ""


def bvid_from_url(url):
    parsed = urllib.parse.urlparse(url)
    for part in parsed.path.split("/"):
        if part.startswith("BV") and len(part) > 4:
            return part
    return ""


def youtube_id_from_url(url):
    parsed = urllib.parse.urlparse(url)
    if parsed.hostname and parsed.hostname.endswith("youtu.be"):
        return parsed.path.strip("/")
    values = urllib.parse.parse_qs(parsed.query)
    return (values.get("v") or [""])[0]


def handle_search(params):
    query = params.get("query", params.get("q", ""))
    platform = params.get("platform", "youtube")
    limit = max(int(params.get("limit") or 10), 1)
    if not query:
        return []
    if platform == "bilibili":
        return bilibili_search(query, limit)
    if platform not in ("youtube", "soundcloud"):
        return {"error": "unsupported platform: " + platform}

    prefix = "ytsearch" if platform == "youtube" else "scsearch"
    cmd = YTDLP + ["--flat-playlist", "-j", prefix + str(limit) + ":" + query]
    try:
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
    except subprocess.TimeoutExpired:
        return {"error": "timeout"}
    except Exception as error:
        return {"error": str(error)}
    items = []
    for line in result.stdout.strip().split("\n"):
        if not line:
            continue
        try:
            data = json.loads(line)
        except json.JSONDecodeError:
            continue
        if platform == "youtube" and data.get("ie_key") not in (None, "", "Youtube"):
            continue
        item = track_from_ytdlp(platform, data)
        if item:
            items.append(item)
    return items


def handle_resolve(params):
    platform = params.get("platform", "youtube")
    source_id = params.get("source_id", params.get("id", ""))
    if not source_id:
        return {"error": "source_id is required"}
    if platform == "bilibili":
        try:
            return bilibili_resolve(source_id)
        except Exception as error:
            return {"error": "bilibili resolve: " + str(error)}
    if platform == "soundcloud":
        url = source_id if source_id.startswith("http") else "https://soundcloud.com/" + source_id
    elif platform == "youtube":
        url = "https://www.youtube.com/watch?v=" + urllib.parse.quote(source_id)
    else:
        return {"error": "unsupported platform: " + platform}
    try:
        result = subprocess.run(YTDLP + ["-j", url], capture_output=True, text=True, timeout=30)
    except subprocess.TimeoutExpired:
        return {"error": "timeout"}
    except Exception as error:
        return {"error": str(error)}
    try:
        return track_from_ytdlp(platform, json.loads(result.stdout.strip()))
    except Exception as error:
        return {"error": str(error)}


def handle_stream(params):
    platform = params.get("platform", "youtube")
    source_id = params.get("source_id", params.get("id", ""))
    if platform == "youtube":
        url = "https://www.youtube.com/watch?v=" + urllib.parse.quote(source_id)
    elif platform == "bilibili":
        url = "https://www.bilibili.com/video/" + urllib.parse.quote(source_id)
    elif platform == "soundcloud":
        url = source_id if source_id.startswith("http") else "https://soundcloud.com/" + source_id
    else:
        return {"error": "unsupported platform: " + platform}
    return {
        "type": "pipe",
        "cmd": YTDLP + ["-o", "-", "-f", "bestaudio/best", url],
    }


def handle_playlist(params):
    url = params.get("url", "")
    if not url:
        return {"error": "no url"}
    platform = platform_for_url(url)
    try:
        result = subprocess.run(
            YTDLP + ["--flat-playlist", "-J", url],
            capture_output=True,
            text=True,
            timeout=45,
        )
    except subprocess.TimeoutExpired:
        return {"error": "timeout"}
    except Exception as error:
        return {"error": str(error)}
    try:
        data = json.loads(result.stdout.strip())
    except Exception as error:
        return {"error": str(error)}

    entries = data.get("entries") if isinstance(data, dict) else None
    if entries is None:
        entries = [data]

    if not entries:
        if platform == "bilibili":
            bvid = bvid_from_url(url)
            if bvid:
                try:
                    return [bilibili_resolve(bvid)]
                except Exception as error:
                    return {"error": "bilibili resolve: " + str(error)}
        if platform == "youtube":
            video_id = youtube_id_from_url(url)
            if video_id:
                return [handle_resolve({"platform": "youtube", "source_id": video_id})]

    items = []
    for entry in entries:
        if not isinstance(entry, dict):
            continue
        entry_platform = platform or (entry.get("extractor_key") or "").lower()
        if entry_platform in ("bilibili", "youtube"):
            item_platform = entry_platform
        else:
            item_platform = platform or entry_platform
        item = track_from_ytdlp(item_platform, entry)
        if item:
            items.append(item)

    if not items:
        if platform == "bilibili":
            bvid = bvid_from_url(url)
            if bvid:
                try:
                    return [bilibili_resolve(bvid)]
                except Exception as error:
                    return {"error": "bilibili resolve: " + str(error)}
        if platform == "youtube":
            video_id = youtube_id_from_url(url)
            if video_id:
                return [handle_resolve({"platform": "youtube", "source_id": video_id})]
    return items


def parse_vtt(path):
    try:
        with open(path, "r", encoding="utf-8", errors="replace") as handle:
            lines = handle.read().splitlines()
    except Exception:
        return ""

    cues = []
    current_time = None
    current_text = []

    def flush():
        nonlocal current_time, current_text
        if current_time is None or not current_text:
            current_time = None
            current_text = []
            return
        text = " ".join(current_text).strip()
        if not text:
            current_time = None
            current_text = []
            return
        if cues and cues[-1][1] == text:
            current_time = None
            current_text = []
            return
        cues.append((current_time, text))
        current_time = None
        current_text = []

    for raw in lines:
        line = raw.strip()
        if "-->" in line:
            flush()
            current_time = line.split("-->")[0].strip()
            current_text = []
            continue
        if current_time is None:
            continue
        if not line:
            flush()
            continue
        if line.startswith("NOTE") or line.startswith("STYLE") or line.startswith("Kind:") or line.startswith("Language:"):
            continue
        text = clean_html(line)
        if text:
            current_text.append(text)
    flush()

    def format_time(value):
        parts = value.replace(",", ".").split(":")
        try:
            seconds = float(parts[-1])
            minutes = int(parts[-2]) if len(parts) >= 2 else 0
            hours = int(parts[-3]) if len(parts) >= 3 else 0
            total = hours * 3600 + minutes * 60 + seconds
            return "[%02d:%05.2f]" % (int(total // 60), total % 60)
        except Exception:
            return ""

    output = []
    for timestamp, text in cues:
        tag = format_time(timestamp)
        if tag:
            output.append(tag + text)
    return "\n".join(output)


def handle_lyrics(params):
    platform = params.get("platform", "youtube")
    source_id = params.get("source_id", params.get("id", ""))
    if not source_id:
        return None
    if platform == "youtube":
        url = "https://www.youtube.com/watch?v=" + urllib.parse.quote(source_id)
    elif platform == "bilibili":
        url = "https://www.bilibili.com/video/" + urllib.parse.quote(source_id)
    else:
        return None

    try:
        with tempfile.TemporaryDirectory(prefix="vide-subs-") as tmp:
            template = os.path.join(tmp, "%(id)s.%(ext)s")
            cmd = YTDLP + [
                "--skip-download",
                "--write-subs",
                "--write-auto-subs",
                "--sub-langs",
                "zh,zh-CN,en",
                "--sub-format",
                "vtt",
                "--no-playlist",
                "-o",
                template,
                url,
            ]
            subprocess.run(cmd, capture_output=True, text=True, timeout=20)
            files = [name for name in os.listdir(tmp) if name.endswith(".vtt")]
            if not files:
                return None
            text = parse_vtt(os.path.join(tmp, files[0]))
            if not text:
                return None
            return {"text": text, "format": "lrc", "language": "auto"}
    except Exception:
        return None


def main():
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
            print(json.dumps({"id": req_id, "error": "not_supported"}), flush=True)
            continue

        if isinstance(result, dict) and "error" in result:
            response = {"id": req_id, "error": result["error"]}
        else:
            response = {"id": req_id, "result": result}
        print(json.dumps(response), flush=True)


if __name__ == "__main__":
    main()
