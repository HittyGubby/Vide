<script lang="ts">
  import SettingsGroup from "$lib/components/settings/ui/SettingsGroup.svelte";
  import SettingCard from "$lib/components/settings/ui/SettingCard.svelte";
  import SettingToggle from "$lib/components/settings/ui/SettingToggle.svelte";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import type { AppSettings } from "$lib/types";

  async function toggle<K extends keyof AppSettings>(key: K, value: AppSettings[K]) {
    await settingsStore.update(key, value);
  }
</script>

<SettingsGroup id="general" title="通用">
  <SettingCard>
    <SettingToggle
      label="关闭到系统托盘"
      checked={settingsStore.settings.close_to_tray}
      onchange={(value) => void toggle("close_to_tray", value)}
    />
    <SettingToggle
      label="启动时自动播放音乐"
      checked={settingsStore.settings.auto_play_on_start}
      onchange={(value) => void toggle("auto_play_on_start", value)}
    />
    <SettingToggle
      label="播放设备改变时自动暂停"
      checked={settingsStore.settings.pause_on_device_change}
      onchange={(value) => void toggle("pause_on_device_change", value)}
    />
  </SettingCard>
</SettingsGroup>
