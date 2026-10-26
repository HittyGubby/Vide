<script lang="ts">
  import SettingsGroup from "$lib/components/settings/ui/SettingsGroup.svelte";
  import SettingCard from "$lib/components/settings/ui/SettingCard.svelte";
  import SettingRow from "$lib/components/settings/ui/SettingRow.svelte";
  import SettingToggle from "$lib/components/settings/ui/SettingToggle.svelte";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import type { AppSettings } from "$lib/types";

  const positions = [
    { id: "top", label: "顶部" },
    { id: "center", label: "中部" },
    { id: "bottom", label: "下部" },
  ];

  async function toggle<K extends keyof AppSettings>(key: K, value: AppSettings[K]) {
    await settingsStore.update(key, value);
  }
</script>

<SettingsGroup id="desktop-lyrics" title="桌面歌词">
  <SettingCard>
    <SettingToggle
      label="展示桌面歌词窗口"
      checked={settingsStore.settings.desktop_lyrics_enabled}
      onchange={(value) => void toggle("desktop_lyrics_enabled", value)}
    />
    <SettingToggle
      label="保持歌词窗口在最顶端"
      checked={settingsStore.settings.desktop_lyrics_always_on_top}
      onchange={(value) => void toggle("desktop_lyrics_always_on_top", value)}
    />
    <SettingToggle
      label="锁定歌词窗口"
      checked={settingsStore.settings.desktop_lyrics_locked}
      onchange={(value) => void toggle("desktop_lyrics_locked", value)}
    />
    <p class="hint">锁定后窗口不再响应拖拽与缩放，并且所有输入事件都会透穿到下层的窗口。</p>

    <SettingRow label="正在播放的歌词行位置">
      <div class="radio-group">
        {#each positions as opt}
          <label class="radio-opt">
            <input
              type="radio"
              name="desktop_lyrics_position"
              value={opt.id}
              checked={settingsStore.settings.desktop_lyrics_position === opt.id}
              onchange={() => void toggle("desktop_lyrics_position", opt.id as AppSettings["desktop_lyrics_position"])}
            />
            <span>{opt.label}</span>
          </label>
        {/each}
      </div>
    </SettingRow>
  </SettingCard>
</SettingsGroup>

<style>
  .hint {
    margin: 0;
    padding: 2px 0 6px;
    font-size: 11px;
    line-height: 1.5;
    color: color-mix(in oklch, var(--color-base-content) 55%, transparent);
  }
  .radio-group {
    display: flex;
    gap: 12px;
    flex-wrap: wrap;
  }
  .radio-opt {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 12px;
    cursor: pointer;
    color: color-mix(in oklch, var(--color-base-content) 80%, transparent);
  }
  .radio-opt input {
    accent-color: var(--color-primary);
  }
</style>
