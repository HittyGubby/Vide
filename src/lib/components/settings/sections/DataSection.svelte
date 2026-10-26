<script lang="ts">
  import { Download, Upload } from "lucide-svelte";
  import SettingsGroup from "$lib/components/settings/ui/SettingsGroup.svelte";
  import SettingCard from "$lib/components/settings/ui/SettingCard.svelte";
  import { invoke } from "$lib/transport";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { canManage } from "$lib/stores/auth.svelte";
  import { applyAppearance } from "$lib/theme";
  import type { AppSettings } from "$lib/types";

  let exporting = $state(false);
  let importMessage = $state<string | null>(null);

  function stamp(): string {
    return new Date().toISOString().slice(0, 10);
  }

  function downloadJson(json: string, name: string) {
    const blob = new Blob([json], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = name;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  }

  async function exportAllSongs() {
    exporting = true;
    importMessage = null;
    try {
      const songs = await invoke("export_songs");
      downloadJson(JSON.stringify(songs, null, 2), "vide_songs_" + stamp() + ".json");
    } catch (e) {
      console.error("export error:", e);
      importMessage = "导出失败";
    } finally {
      exporting = false;
    }
  }

  function exportSettings() {
    downloadJson(
      JSON.stringify(settingsStore.settings, null, 2),
      "vide_settings_" + stamp() + ".json",
    );
  }

  function importJson(e: Event, mode: "songs" | "settings") {
    const input = e.target as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      try {
        const parsed = JSON.parse(String(reader.result));
        if (mode === "settings") {
          if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
            settingsStore.settings = { ...settingsStore.settings, ...parsed } as AppSettings;
            applyAppearance(settingsStore.settings);
            void settingsStore.save();
            importMessage = "设置已导入";
          } else {
            importMessage = "无法识别为设置";
          }
          return;
        }
        if (Array.isArray(parsed) && parsed.every((s) => s && typeof s === "object" && "id" in s)) {
          void importSongs(parsed);
        } else {
          importMessage = "无法识别为歌曲数据";
        }
      } catch {
        importMessage = "导入失败：无法解析 JSON";
      }
    };
    reader.readAsText(file);
    input.value = "";
  }

  async function importSongs(songs: unknown[]) {
    try {
      const n = await invoke<number>("import_songs", { songs });
      importMessage = "已导入 " + n + " 首歌曲";
    } catch (e) {
      console.error("import songs error:", e);
      importMessage = "导入歌曲失败";
    }
  }
</script>

<SettingsGroup id="data" title="数据" description="导出/导入歌曲与设置（JSON 文件）。导入设置会立即应用主题与字体。">
  <SettingCard>
    <div class="btn-row">
      <button class="btn-secondary" onclick={exportAllSongs} disabled={exporting}>
        <Download size={14} />
        <span>{exporting ? "导出中..." : "导出歌曲"}</span>
      </button>
      {#if canManage()}
        <label class="btn-secondary">
          <Upload size={14} />
          <span>导入歌曲</span>
          <input type="file" accept=".json" class="hidden-file" onchange={(e) => importJson(e, "songs")} />
        </label>
      {/if}
      <button class="btn-secondary" onclick={exportSettings}>
        <Download size={14} />
        <span>导出设置</span>
      </button>
      {#if canManage()}
        <label class="btn-secondary">
          <Upload size={14} />
          <span>导入设置</span>
          <input type="file" accept=".json" class="hidden-file" onchange={(e) => importJson(e, "settings")} />
        </label>
      {/if}
    </div>
    {#if importMessage}
      <p class="message">{importMessage}</p>
    {/if}
  </SettingCard>
</SettingsGroup>

<style>
  .btn-row {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    padding: 6px 0;
  }
  .btn-secondary {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 6px 14px;
    border-radius: 6px;
    background: var(--color-base-300);
    color: var(--color-base-content);
    font-size: 12px;
    border: none;
    cursor: pointer;
  }
  .btn-secondary:hover {
    background: var(--color-base-100);
  }
  .btn-secondary:disabled {
    opacity: 0.5;
  }
  .hidden-file {
    display: none;
  }
  .message {
    margin: 0;
    padding: 2px 0;
    font-size: 12px;
    color: var(--color-primary);
  }
</style>
