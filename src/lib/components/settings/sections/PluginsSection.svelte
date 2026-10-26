<script lang="ts">
  import SettingsGroup from "$lib/components/settings/ui/SettingsGroup.svelte";
  import PluginSettings from "$lib/components/settings/PluginSettings.svelte";
</script>

<SettingsGroup
  id="plugins"
  title="插件"
  description="把插件文件夹放进插件目录后点「重新扫描」，无需重启应用。每个平台每种能力只选一个 provider，顺序就是 fallback 顺序。"
>
  <PluginSettings />
</SettingsGroup>
