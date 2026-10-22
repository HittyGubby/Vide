# yt-dlp adapter

Vide plugin for YouTube, Bilibili, and SoundCloud.

- search: YouTube/SoundCloud via yt-dlp search; Bilibili via its web search API
- resolve: yt-dlp metadata or Bilibili view API
- stream: yt-dlp pipe (bestaudio)
- playlist: yt-dlp flat playlist; a single video/track URL returns one Track
- lyrics: YouTube/Bilibili auto subtitles (SoundCloud has none)

## Requirements

- Python 3 on PATH
- yt-dlp on PATH
- yt-dlp may need cookies for age/region/login-restricted content and
  Bilibili playlists/favorites. Put yt-dlp options in the plugin argv:

    python3
    adapter.py
    --cookies-from-browser
    chrome

Each option is one argv element. The adapter passes everything after
adapter.py to yt-dlp.

## Notes

- Bilibili search creates a temporary cookie jar by visiting the homepage,
  which is enough for the public search endpoint.
- Bilibili playlists/favorites may still require a logged-in cookie.
- YouTube results are filtered to video entries, not channels.
