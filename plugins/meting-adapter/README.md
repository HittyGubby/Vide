# Meting adapter

Vide plugin for:

- NetEase Cloud Music (netease)
- Tencent Music (tencent)
- Kugou (kugou)
- Kuwo (kuwo)

It uses a vendored copy of Meting (MIT) for search/metadata/playlist/lyrics.
Kuwo search/stream use the older public endpoints because Meting's current
Kuwo web API returns "request is illegal".

## Requirements

- Node.js 18+ on PATH (not bundled by Vide)
- Network access to the platform endpoints

Install by placing this folder under Vide's plugin directory and clicking
"重新扫描" in Settings -> 插件.

## Stream URLs

- NetEase usually works without a cookie.
- Kuwo uses the old antiserver.kuwo.cn endpoint.
- Tencent and Kugou often return no URL without a logged-in cookie.

To supply a cookie, edit the plugin argv in the plugin settings and add:

    node adapter.mjs --cookie "<cookie string>"

Each option is one argv element; for example:

    node
    adapter.mjs
    --cookie
    uin=...; qm_keyst=...

The same cookie is passed to Meting for all platforms, so use a platform whose
cookie is meaningful to the platform you are playing.

## Limitations

- Search results do not include durations/covers yet.
- Tencent/Kugou stream URLs depend on the account/cookie and platform policy.
- Kuwo lyrics are currently not implemented.
- Lyrics are returned as LRC text.

## License

The vendored Meting library is MIT licensed; see METING_LICENSE.
