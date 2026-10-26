<script lang="ts">
  import SettingsGroup from "$lib/components/settings/ui/SettingsGroup.svelte";
  import SettingCard from "$lib/components/settings/ui/SettingCard.svelte";
  import CssEditor from "$lib/components/settings/CssEditor.svelte";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { platform } from "$lib/platform.svelte";
  import { isTauri } from "$lib/isTauri";

  async function setCss(key: "desktop_lyrics_css" | "detail_lyrics_css", value: string) {
    await settingsStore.update(key, value);
  }

  let editorHeight = $derived(platform.mobileUI ? 220 : 300);
</script>

<SettingsGroup
  id="lyrics-style"
  title="歌词样式"
  description="两段 CSS 分别作用于悬浮歌词窗口和播放详情页的歌词区域，保存后立即生效。自动滚动由「桌面歌词」里的位置控制，其余外观交给这里的样式。"
>
  <SettingCard>
    {#if isTauri}
      <p class="editor-label">悬浮歌词样式</p>
      <CssEditor
        value={settingsStore.settings.desktop_lyrics_css}
        onchange={(next) => void setCss("desktop_lyrics_css", next)}
        heightPx={editorHeight}
      />
    {/if}
    <p class="editor-label">播放详情歌词样式</p>
    <CssEditor
      value={settingsStore.settings.detail_lyrics_css}
      onchange={(next) => void setCss("detail_lyrics_css", next)}
      heightPx={editorHeight}
    />
  </SettingCard>
</SettingsGroup>

<style>
  .editor-label {
    margin: 8px 2px 2px;
    font-size: 12px;
    color: color-mix(in oklch, var(--color-base-content) 70%, transparent);
  }
</style>
