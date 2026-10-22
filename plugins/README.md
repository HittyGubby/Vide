# Vide plugins

Current adapters:

| Folder | Platforms | Capabilities |
|---|---|---|
| meting-adapter | netease, tencent, kugou, kuwo | search, resolve, stream, playlist, lyrics (Kuwo lyrics null) |
| ytdlp-adapter | youtube, bilibili, soundcloud | search, resolve, stream, playlist; YouTube/Bilibili subtitles, no SoundCloud lyrics |

Install: copy the folder into the plugin directory shown in Settings -> 插件,
then click "重新扫描". Node.js 18+ is required for meting-adapter; Python 3 +
yt-dlp for ytdlp-adapter. Neither runtime is bundled.

Limitations:

- Tencent/Kugou stream URLs often need a logged-in cookie. Add
  --cookie "<cookie>" to meting-adapter's argv.
- Kuwo lyrics are not implemented.
- YouTube/Bilibili lyrics are fetched from subtitles when available; SoundCloud has no lyrics.
- Search results from meting-adapter include duration/cover where the platform exposes them.
- Migu and Baidu are intentionally not included.
