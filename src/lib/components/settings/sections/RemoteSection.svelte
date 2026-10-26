<script lang="ts">
  import SettingsGroup from "$lib/components/settings/ui/SettingsGroup.svelte";
  import SettingCard from "$lib/components/settings/ui/SettingCard.svelte";
  import RemoteConnection from "$lib/components/settings/RemoteConnection.svelte";
</script>

<SettingsGroup
  id="remote"
  title="远程连接"
  description="连接 vide-server：游客只读，登录后可播放控制、编辑与管理服务端。"
>
  <SettingCard>
    <RemoteConnection />
  </SettingCard>
</SettingsGroup>
