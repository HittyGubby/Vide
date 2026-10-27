<script lang="ts">
  import SettingsGroup from "$lib/components/settings/ui/SettingsGroup.svelte";
  import SettingCard from "$lib/components/settings/ui/SettingCard.svelte";
  import SettingRow from "$lib/components/settings/ui/SettingRow.svelte";
  import SettingToggle from "$lib/components/settings/ui/SettingToggle.svelte";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import type { AppSettings } from "$lib/types";

  async function toggle<K extends keyof AppSettings>(key: K, value: AppSettings[K]) {
    await settingsStore.update(key, value);
  }

  async function setTarget(value: string) {
    const parsed = Number(value);
    if (!Number.isFinite(parsed)) return;
    await settingsStore.update(
      "loudness_target_lufs",
      Math.max(-30, Math.min(-6, parsed)),
    );
  }
</script>

<SettingsGroup
  id="playback"
  title="播放"
  description="静音与响度分析由服务端完成；本地/已缓存文件立即生效，在线流首次播放可能需要等待缓存与分析。"
>
  <SettingCard>
    <SettingToggle
      label="跳过歌头/歌尾静音"
      checked={settingsStore.settings.skip_silence}
      onchange={(value) => void toggle("skip_silence", value)}
    />

    <SettingRow label="音量均衡" hint="按整首平均响度（EBU R128）做固定增益">
      <label class="inline-toggle" title="音量均衡">
        <input
          type="checkbox"
          checked={settingsStore.settings.normalize_loudness}
          onchange={(e) => void toggle("normalize_loudness", e.currentTarget.checked)}
        />
      </label>
      <input
        type="number"
        min="-30"
        max="-6"
        step="1"
        value={settingsStore.settings.loudness_target_lufs}
        onchange={(e) => void setTarget(e.currentTarget.value)}
        disabled={!settingsStore.settings.normalize_loudness}
        class="num-input"
        aria-label="目标响度 LUFS"
      />
      <span class="unit">LUFS</span>
    </SettingRow>
  </SettingCard>
</SettingsGroup>

<style>
  .inline-toggle {
    display: flex;
    align-items: center;
    justify-content: center;
    flex: none;
  }
  .inline-toggle input {
    width: 16px;
    height: 16px;
    accent-color: var(--color-primary);
  }
  .num-input {
    flex: none;
    width: 64px;
    height: 28px;
    padding: 0 6px;
    border-radius: 6px;
    background: var(--color-base-100);
    border: 1px solid var(--panel-border);
    color: var(--color-base-content);
    font-size: 12px;
    text-align: center;
    user-select: text;
  }
  .num-input:focus {
    outline: none;
    border-color: var(--color-primary);
  }
  .num-input:disabled {
    opacity: 0.45;
  }
  .unit {
    flex: none;
    font-size: 10px;
    color: color-mix(in oklch, var(--color-base-content) 55%, transparent);
  }
</style>
