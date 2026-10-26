<script lang="ts">
  import SettingsGroup from "$lib/components/settings/ui/SettingsGroup.svelte";
  import SettingCard from "$lib/components/settings/ui/SettingCard.svelte";
  import SettingRow from "$lib/components/settings/ui/SettingRow.svelte";
  import ShortcutCell from "$lib/components/settings/ShortcutCell.svelte";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { SHORTCUT_ACTIONS } from "$lib/shortcuts";
  import type { AppSettings, ShortcutAction } from "$lib/types";

  async function setNumber(key: keyof AppSettings, value: string) {
    const n = parseInt(value, 10);
    if (!Number.isNaN(n)) await settingsStore.update(key, n);
  }

  async function updateShortcut(
    action: ShortcutAction,
    field: "in_app" | "global",
    value: string,
  ) {
    const shortcuts = { ...settingsStore.settings.shortcuts };
    shortcuts[action] = { ...shortcuts[action], [field]: value };
    await settingsStore.update("shortcuts", shortcuts);
  }
</script>

<SettingsGroup id="shortcuts" title="快捷键">
  <SettingCard>
    {#each SHORTCUT_ACTIONS as item}
      <div class="shortcut-row">
        <span class="action">{item.label}</span>
        <div class="keys">
          <label class="key">
            <span>应用内</span>
            <ShortcutCell
              value={settingsStore.settings.shortcuts[item.id].in_app}
              onchange={(value) => void updateShortcut(item.id, "in_app", value)}
            />
          </label>
          <label class="key">
            <span>全局</span>
            <ShortcutCell
              value={settingsStore.settings.shortcuts[item.id].global}
              onchange={(value) => void updateShortcut(item.id, "global", value)}
            />
          </label>
        </div>
      </div>
    {/each}

    <p class="hint">
      点击单元格后按下新的组合键即可修改；Backspace 清除，Esc 取消。全局快捷键在应用未获得焦点时同样生效。
    </p>
    {#if settingsStore.globalShortcutFailures.length > 0}
      <p class="warning">
        系统未接受以下全局快捷键（可能已被其他程序占用）：
        {settingsStore.globalShortcutFailures.join("；")}
      </p>
    {/if}
  </SettingCard>

  <SettingCard>
    <SettingRow label="快进/快退时长（秒）">
      <input
        type="number"
        min="1"
        max="120"
        step="1"
        value={settingsStore.settings.seek_step_secs}
        onchange={(e) => void setNumber("seek_step_secs", e.currentTarget.value)}
        class="num-input"
      />
    </SettingRow>
    <SettingRow label="音量调节步长">
      <input
        type="number"
        min="1"
        max="50"
        step="1"
        value={settingsStore.settings.volume_step}
        onchange={(e) => void setNumber("volume_step", e.currentTarget.value)}
        class="num-input"
      />
    </SettingRow>
  </SettingCard>
</SettingsGroup>

<style>
  .shortcut-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 7px 0;
    border-bottom: 1px solid var(--panel-border);
    font-size: 13px;
  }
  .shortcut-row:last-of-type {
    border-bottom: none;
  }
  .action {
    min-width: 0;
  }
  .keys {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .key {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 10px;
    color: color-mix(in oklch, var(--color-base-content) 55%, transparent);
  }
  .hint {
    margin: 0;
    padding: 6px 0 2px;
    font-size: 11px;
    line-height: 1.6;
    color: color-mix(in oklch, var(--color-base-content) 55%, transparent);
  }
  .warning {
    margin: 0;
    padding: 2px 0 6px;
    font-size: 11px;
    line-height: 1.6;
    color: var(--color-warning);
  }
  .num-input {
    width: 100%;
    height: 30px;
    padding: 0 10px;
    border-radius: 6px;
    background: var(--color-base-100);
    border: 1px solid var(--panel-border);
    color: var(--color-base-content);
    font-size: 13px;
    text-align: center;
    user-select: text;
  }
  .num-input:focus {
    outline: none;
    border-color: var(--color-primary);
  }

  @media (max-width: 560px) {
    .shortcut-row {
      align-items: flex-start;
      flex-direction: column;
      gap: 6px;
    }
    .keys {
      width: 100%;
      justify-content: space-between;
    }
  }
</style>
