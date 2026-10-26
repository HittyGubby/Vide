<script lang="ts">
  import SettingsGroup from "$lib/components/settings/ui/SettingsGroup.svelte";
  import SettingCard from "$lib/components/settings/ui/SettingCard.svelte";
  import SettingRow from "$lib/components/settings/ui/SettingRow.svelte";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { isTauri } from "$lib/isTauri";
  import { DEFAULT_PRIMARY } from "$lib/theme";
  import type { AppSettings } from "$lib/types";

  async function setNumber(key: keyof AppSettings, value: string) {
    const n = parseInt(value, 10);
    if (!Number.isNaN(n)) await settingsStore.update(key, n);
  }

  async function setText(key: keyof AppSettings, value: string) {
    await settingsStore.update(key, value);
  }
</script>

<SettingsGroup id="appearance" title="外观">
  <SettingCard>
    {#if isTauri}
      <SettingRow label="窗口缩放" hint="百分比，50–200">
        <input
          type="number"
          min="50"
          max="200"
          step="5"
          value={settingsStore.settings.window_scale}
          onchange={(e) => void setNumber("window_scale", e.currentTarget.value)}
          class="num-input"
        />
      </SettingRow>
    {/if}

    <SettingRow label="主题色">
      <div class="color-row">
        <input
          type="color"
          value={settingsStore.settings.theme_primary}
          oninput={(e) => void setText("theme_primary", e.currentTarget.value)}
          class="color-input"
        />
        <input
          type="text"
          value={settingsStore.settings.theme_primary}
          onchange={(e) => void setText("theme_primary", e.currentTarget.value)}
          placeholder="#5f5dfd"
          class="text-input color-text"
        />
        <button class="clear-btn" onclick={() => void setText("theme_primary", DEFAULT_PRIMARY)}>
          重置
        </button>
      </div>
    </SettingRow>

    <SettingRow label="字体">
      <input
        type="text"
        value={settingsStore.settings.font_family}
        onchange={(e) => void setText("font_family", e.currentTarget.value)}
        placeholder="Segoe UI, sans-serif"
        class="text-input"
      />
    </SettingRow>
  </SettingCard>
</SettingsGroup>

<style>
  .num-input,
  .text-input {
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
  .num-input {
    max-width: 90px;
  }
  .num-input:focus,
  .text-input:focus {
    outline: none;
    border-color: var(--color-primary);
  }

  .color-row {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
    width: 100%;
  }
  .color-input {
    flex: none;
    width: 40px;
    height: 28px;
    padding: 0;
    border: 1px solid var(--panel-border);
    border-radius: 6px;
    background: none;
    cursor: pointer;
  }
  .text-input.color-text {
    width: calc(7ch + 14px);
    padding: 0 6px;
    text-align: center;
    font-family: ui-monospace, "SFMono-Regular", Menlo, Consolas, monospace;
  }
  .clear-btn {
    flex: none;
    padding: 4px 10px;
    border: 1px solid var(--panel-border-strong, var(--panel-border));
    border-radius: 6px;
    background: var(--color-base-100);
    color: var(--color-base-content);
    font-size: 12px;
    cursor: pointer;
  }
  .clear-btn:hover {
    border-color: var(--color-primary);
    color: var(--color-primary);
  }

  @media (max-width: 560px) {
    .color-row {
      justify-content: flex-start;
    }
    .num-input {
      max-width: none;
    }
  }
</style>
