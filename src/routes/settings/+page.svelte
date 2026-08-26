<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { Settings as SettingsIcon, Scan, Download, Upload, FolderPlus, Edit, X, Check } from "lucide-svelte";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import type { AppSettings } from "$lib/types";

  let roots = $state<string[]>([]);
  let excludeRoots = $state<string[]>([]);
  let newRootPath = $state("");
  let newExcludePath = $state("");
  let showInput = $state(false);
  let showExcludeInput = $state(false);
  let editingInclude = $state<string | null>(null);
  let editingExclude = $state<string | null>(null);
  let scanning = $state(false);
  let scanResult = $state<{ scanned: number; added: number; skipped: number; errors: string[] } | null>(null);
  let exporting = $state(false);
  let exportDone = $state(false);
  let importMessage = $state<string | null>(null);

  onMount(async () => {
    await settingsStore.load();
    try {
      roots = await invoke("get_library_roots");
      excludeRoots = await invoke("get_exclude_roots");
    } catch {
      roots = [];
      excludeRoots = [];
    }
  });

  const alignOptions = [
    { id: "left", label: "左侧对齐" },
    { id: "center", label: "居中" },
    { id: "right", label: "右侧对齐" },
  ];

  async function toggle(key: keyof AppSettings, value: boolean) {
    await settingsStore.update(key, value);
  }

  async function setNumber(key: keyof AppSettings, value: string) {
    const n = parseInt(value, 10);
    if (isNaN(n)) return;
    await settingsStore.update(key, n);
  }

  async function setText(key: keyof AppSettings, value: string) {
    await settingsStore.update(key, value);
  }

  // ── Included library roots (CRUD) ─────────────
  async function addRoot() {
    const path = newRootPath.trim();
    if (!path) return;
    try {
      await invoke("add_library_root", { path });
      roots = await invoke("get_library_roots");
      newRootPath = "";
      showInput = false;
    } catch (e) {
      console.error("add root error:", e);
    }
  }
  async function removeRoot(path: string) {
    try {
      await invoke("remove_library_root", { path });
      roots = await invoke("get_library_roots");
    } catch (e) {
      console.error("remove root error:", e);
    }
  }
  function startEditInclude(path: string) {
    editingInclude = path;
    newRootPath = path;
  }
  async function saveEditInclude() {
    if (!editingInclude) return;
    const newPath = newRootPath.trim();
    const oldPath = editingInclude;
    if (!newPath) { editingInclude = null; return; }
    try {
      if (newPath !== oldPath) {
        await invoke("remove_library_root", { path: oldPath });
        await invoke("add_library_root", { path: newPath });
        roots = await invoke("get_library_roots");
      }
      editingInclude = null;
      newRootPath = "";
    } catch (e) {
      console.error("edit root error:", e);
    }
  }

  // ── Excluded folder list (CRUD) ───────────────
  async function addExclude() {
    const path = newExcludePath.trim();
    if (!path) return;
    try {
      await invoke("add_exclude_root", { path });
      excludeRoots = await invoke("get_exclude_roots");
      newExcludePath = "";
      showExcludeInput = false;
    } catch (e) {
      console.error("add exclude error:", e);
    }
  }
  async function removeExclude(path: string) {
    try {
      await invoke("remove_exclude_root", { path });
      excludeRoots = await invoke("get_exclude_roots");
    } catch (e) {
      console.error("remove exclude error:", e);
    }
  }
  function startEditExclude(path: string) {
    editingExclude = path;
    newExcludePath = path;
  }
  async function saveEditExclude() {
    if (!editingExclude) return;
    const newPath = newExcludePath.trim();
    const oldPath = editingExclude;
    if (!newPath) { editingExclude = null; return; }
    try {
      if (newPath !== oldPath) {
        await invoke("remove_exclude_root", { path: oldPath });
        await invoke("add_exclude_root", { path: newPath });
        excludeRoots = await invoke("get_exclude_roots");
      }
      editingExclude = null;
      newExcludePath = "";
    } catch (e) {
      console.error("edit exclude error:", e);
    }
  }

  async function scanLibrary() {
    scanning = true;
    scanResult = null;
    try {
      const result = await invoke("scan_library");
      scanResult = result as any;
    } catch (e) {
      console.error("scan error:", e);
    } finally {
      scanning = false;
    }
  }

  // ── Data export/import ──────────────────────
  async function exportAllSongs() {
    exporting = true;
    exportDone = false;
    importMessage = null;
    try {
      const songs = await invoke("list_songs");
      const json = JSON.stringify(songs, null, 2);
      downloadJson(json, "vide_songs_" + stamp() + ".json");
      exportDone = true;
    } catch (e) {
      console.error("export error:", e);
    } finally {
      exporting = false;
    }
  }

  async function exportSettings() {
    const json = JSON.stringify(settingsStore.settings, null, 2);
    downloadJson(json, "vide_settings_" + stamp() + ".json");
  }

  function importJson(e: Event, mode: "songs" | "settings") {
    const input = e.target as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      try {
        const data = JSON.parse(String(reader.result));
        if (mode === "settings") {
          if (data && typeof data === "object" && !Array.isArray(data)) {
            settingsStore.settings = { ...settingsStore.settings, ...data };
            settingsStore.save();
            importMessage = "设置已导入";
          } else {
            importMessage = "无法识别为设置";
          }
        } else {
          // songs mode
          if (Array.isArray(data) && data.every((s) => s && typeof s === "object" && "id" in s)) {
            importSongs(data);
          } else {
            importMessage = "无法识别为歌曲数据";
          }
        }
      } catch {
        importMessage = "导入失败：无法解析 JSON";
      }
    };
    reader.readAsText(file);
    input.value = "";
  }

  async function importSongs(songs: any[]) {
    try {
      const n = await invoke<number>("import_songs", { songs });
      importMessage = "已导入 " + n + " 首歌曲";
    } catch (e) {
      console.error("import songs error:", e);
      importMessage = "导入歌曲失败";
    }
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

  function stamp() {
    return new Date().toISOString().slice(0, 10);
  }
</script>

<div class="settings-view">
  <div class="settings-header">
    <SettingsIcon size={18} />
    <h1>设置</h1>
  </div>

  <!-- 通用 -->
  <section class="group">
    <h2 class="group-title">通用</h2>
    <div class="setting-card">
      <label class="row checkbox-row">
        <input type="checkbox" checked={settingsStore.settings.close_to_tray} onchange={(e) => toggle("close_to_tray", (e.currentTarget as HTMLInputElement).checked)} />
        <span>关闭到系统托盘</span>
      </label>
      <label class="row checkbox-row">
        <input type="checkbox" checked={settingsStore.settings.auto_play_on_start} onchange={(e) => toggle("auto_play_on_start", (e.currentTarget as HTMLInputElement).checked)} />
        <span>启动时自动播放音乐</span>
      </label>
      <label class="row checkbox-row">
        <input type="checkbox" checked={settingsStore.settings.pause_on_device_change} onchange={(e) => toggle("pause_on_device_change", (e.currentTarget as HTMLInputElement).checked)} />
        <span>播放设备改变时自动暂停</span>
      </label>
    </div>
  </section>

  <!-- 外观 -->
  <section class="group">
    <h2 class="group-title">外观</h2>
    <div class="setting-card">
      <div class="row input-row">
        <span class="row-label">窗口缩放</span>
        <input
          type="number"
          min="50"
          max="200"
          step="5"
          value={settingsStore.settings.window_scale}
          onchange={(e) => setNumber("window_scale", (e.currentTarget as HTMLInputElement).value)}
          class="num-input"
        />
      </div>
      <div class="row input-row">
        <span class="row-label">字体</span>
        <input
          type="text"
          value={settingsStore.settings.font_family}
          onchange={(e) => setText("font_family", (e.currentTarget as HTMLInputElement).value)}
          placeholder="Segoe UI, sans-serif"
          class="text-input"
        />
      </div>
    </div>
  </section>

  <!-- 歌词 -->
  <section class="group">
    <h2 class="group-title">歌词</h2>
    <div class="setting-card">
      <div class="row radio-row">
        <span class="row-label">歌词靠侧</span>
        <div class="radio-group">
          {#each alignOptions as opt}
            <label class="radio-opt">
              <input
                type="radio"
                name="lyrics_align"
                value={opt.id}
                checked={settingsStore.settings.lyrics_align === opt.id}
                onchange={() => setText("lyrics_align", opt.id)}
              />
              <span>{opt.label}</span>
            </label>
          {/each}
        </div>
      </div>
      <div class="row input-row">
        <span class="row-label">歌词字号</span>
        <input
          type="number"
          min="10"
          max="40"
          value={settingsStore.settings.lyrics_font_size}
          onchange={(e) => setNumber("lyrics_font_size", (e.currentTarget as HTMLInputElement).value)}
          class="num-input"
        />
      </div>
    </div>
  </section>

  <!-- 库管理 -->
  <section class="group">
    <h2 class="group-title">库管理
      <button class="btn-secondary" onclick={scanLibrary} disabled={scanning || roots.length === 0}>
        <Scan size={14} /><span>{scanning ? "扫描中..." : "立即扫描"}</span>
      </button>
    </h2>


    <!-- Included folders -->
    <div class="setting-card">
      <div class="row">
        <span class="row-label">包含文件夹</span>
        <div class="btn-group">
          <button class="btn-secondary" onclick={() => (showInput = !showInput)}>
            <FolderPlus size={13} /><span>添加</span>
          </button>

        </div>
      </div>
      {#if showInput}
        <div class="add-root-row">
          <input type="text" class="root-input" placeholder="/path/to/music" bind:value={newRootPath}
            onkeydown={(e) => { if (e.key === "Enter") addRoot(); }} />
          <button class="btn-add" onclick={addRoot} disabled={!newRootPath.trim()}>确定</button>
        </div>
      {/if}
      <div class="root-list">
        {#if roots.length === 0}
          <div class="root-empty">暂未添加包含目录</div>
        {:else}
          {#each roots as root}
            <div class="root-item">
              {#if editingInclude === root}
                <input type="text" class="root-edit" bind:value={newRootPath} onkeydown={(e) => { if (e.key === "Enter") saveEditInclude(); if (e.key === "Escape") { editingInclude = null; } }} />
                <button class="root-btn ok" onclick={saveEditInclude}><Check size={13} /></button>
                <button class="root-btn" onclick={() => { editingInclude = null; newRootPath = ""; }}><X size={13} /></button>
              {:else}
                <span class="root-path">{root}</span>
                <button class="root-btn" onclick={(e) => { e.stopPropagation(); startEditInclude(root); }} title="编辑"><Edit size={13} /></button>
                <button class="root-btn" onclick={(e) => { e.stopPropagation(); removeRoot(root); }} title="删除"><X size={13} /></button>
              {/if}
            </div>
          {/each}
        {/if}
      </div>
    </div>

    <!-- Excluded folders -->
    <div class="setting-card">
      <div class="row">
        <span class="row-label">排除文件夹</span>
        <button class="btn-secondary" onclick={() => (showExcludeInput = !showExcludeInput)}>
          <FolderPlus size={13} /><span>添加</span>
        </button>
      </div>
      {#if showExcludeInput}
        <div class="add-root-row">
          <input type="text" class="root-input" placeholder="/path/to/exclude" bind:value={newExcludePath}
            onkeydown={(e) => { if (e.key === "Enter") addExclude(); }} />
          <button class="btn-add" onclick={addExclude} disabled={!newExcludePath.trim()}>确定</button>
        </div>
      {/if}
      <div class="root-list">
        {#if excludeRoots.length === 0}
          <div class="root-empty">暂无排除目录</div>
        {:else}
          {#each excludeRoots as path}
            <div class="root-item">
              {#if editingExclude === path}
                <input type="text" class="root-edit" bind:value={newExcludePath} onkeydown={(e) => { if (e.key === "Enter") saveEditExclude(); if (e.key === "Escape") { editingExclude = null; } }} />
                <button class="root-btn ok" onclick={saveEditExclude}><Check size={13} /></button>
                <button class="root-btn" onclick={() => { editingExclude = null; newExcludePath = ""; }}><X size={13} /></button>
              {:else}
                <span class="root-path">{path}</span>
                <button class="root-btn" onclick={(e) => { e.stopPropagation(); startEditExclude(path); }} title="编辑"><Edit size={13} /></button>
                <button class="root-btn" onclick={(e) => { e.stopPropagation(); removeExclude(path); }} title="删除"><X size={13} /></button>
              {/if}
            </div>
          {/each}
        {/if}
      </div>
    </div>

    {#if scanResult}
      <div class="scan-result">扫描完成: {scanResult.scanned} 个文件, 新增 {scanResult.added} 首, 跳过 {scanResult.skipped} 首{#if scanResult.errors.length > 0} · {scanResult.errors.length} 个错误{/if}</div>
    {/if}
  </section>

  <!-- 数据 -->
  <section class="group">
    <h2 class="group-title">数据</h2>
      <div class="btn-row inline">
        <button class="btn-secondary" onclick={exportAllSongs} disabled={exporting}>
          <Download size={13} /><span>{exporting ? "导出中..." : "导出歌曲"}</span>
        </button>
        <label class="btn-secondary">
          <Upload size={13} /><span>导入歌曲</span>
          <input type="file" accept=".json" class="hidden-file" onchange={(e) => importJson(e, "songs")} />
        </label>
        <button class="btn-secondary" onclick={exportSettings}>
          <Download size={13} /><span>导出设置</span>
        </button>
        <label class="btn-secondary">
          <Upload size={13} /><span>导入设置</span>
          <input type="file" accept=".json" class="hidden-file" onchange={(e) => importJson(e, "settings")} />
        </label>
      </div>
      {#if importMessage}
        <div class="import-message">{importMessage}</div>
      {/if}
  </section>
</div>

<style>
  .settings-view {
    max-width: 700px;
    padding: 24px 32px;
    display: flex;
    flex-direction: column;
    gap: 24px;
  }

  .settings-header {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--color-base-content);
  }
  .settings-header h1 { font-size: 18px; font-weight: 600; }

  .group { display: flex; flex-direction: column; gap: 8px; }
  .group-title {
    font-size: 14px; font-weight: 700;
    color: var(--color-primary);
    padding-bottom: 6px;
    border-bottom: 1px solid var(--panel-border);
    display: flex;
    justify-content: space-between;
  }
  .group-desc { font-size: 11px; color: var(--color-base-content) / 0.6; margin-bottom: 4px; }

  .setting-card {
    background: var(--color-base-200);
    border: 1px solid var(--panel-border);
    border-radius: 8px;
    padding: 8px 14px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 8px 0;
  }
  .row + .row { border-top: 1px solid var(--panel-border); }

  .row-label { font-size: 13px; color: var(--color-base-content); }

  .checkbox-row { cursor: pointer; }
  .checkbox-row input { accent-color: var(--color-primary); width: 16px; height: 16px; }
  .checkbox-row span { flex: 1; }

  .radio-group { display: flex; gap: 10px; }
  .radio-opt { display: flex; align-items: center; gap: 5px; font-size: 12px; cursor: pointer; color: var(--color-base-content) / 0.8; }
  .radio-opt input { accent-color: var(--color-primary); }

  .num-input {
    width: 90px; height: 30px; padding: 0 10px; border-radius: 6px;
    background: var(--color-base-100); border: 1px solid var(--panel-border);
    color: var(--color-base-content); font-size: 13px; text-align: right; user-select: text;
  }
  .text-input {
    width: 280px; height: 30px; padding: 0 10px; border-radius: 6px;
    background: var(--color-base-100); border: 1px solid var(--panel-border);
    color: var(--color-base-content); font-size: 13px; user-select: text;
  }
  .num-input:focus, .text-input:focus { border-color: var(--color-primary); }

  .btn-secondary {
    display: flex; align-items: center; gap: 4px;
    padding: 5px 14px; border-radius: 6px;
    background: var(--color-base-300);
    color: var(--color-base-content);
    font-size: 12px; border: none; cursor: pointer;
  }
  .btn-secondary:hover { background: var(--color-base-100); color: var(--color-base-content); }
  .btn-secondary:disabled { opacity: 0.5; }

  .btn-primary {
    display: flex; align-items: center; justify-content: center; gap: 6px;
    padding: 8px 16px; border-radius: 6px;
    background: var(--color-primary); color: var(--color-primary-content);
    font-size: 12px; font-weight: 500; border: none; align-self: flex-start;
  }
  .btn-primary:hover:not(:disabled) { opacity: 0.9; }
  .btn-primary:disabled { opacity: 0.5; }

  .add-root-row { display: flex; gap: 6px; margin-top: 6px; }
  .root-input {
    flex: 1; padding: 6px 10px; border-radius: 6px;
    background: var(--color-base-100); border: 1px solid var(--panel-border);
    color: var(--color-base-content); font-size: 12px; user-select: text;
  }
  .btn-add {
    padding: 6px 14px; border-radius: 6px;
    background: var(--color-primary); color: var(--color-primary-content);
    font-size: 12px; border: none;
  }
  .btn-add:disabled { opacity: 0.5; }

  .root-list { margin-top: 8px; display: flex; flex-direction: column; gap: 2px; }
  .root-empty { font-size: 12px; color: var(--color-base-content) / 0.5; }
  .root-item {
    display: flex; align-items: center; gap: 6px;
    font-size: 12px; color: var(--color-base-content) / 0.85;
    font-family: monospace; user-select: text;
    padding: 2px 0;
  }
  .root-path { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .root-btn {
    flex: none; display: flex; align-items: center; justify-content: center;
    width: 22px; height: 22px; border-radius: 4px;
    background: none; border: none; color: var(--color-base-content) / 0.5;
  }
  .root-btn:hover { background: var(--color-base-300); color: var(--color-base-content); }
  .root-btn.ok { color: var(--color-success); }
  .root-edit {
    flex: 1; min-width: 0; height: 24px; padding: 0 8px;
    background: var(--color-base-100); border: 1px solid var(--panel-border);
    border-radius: 4px; color: var(--color-base-content); font-size: 12px;
    font-family: monospace; user-select: text;
  }

  .btn-group { display: flex; align-items: center; gap: 6px; }

  .scan-result { font-size: 11px; color: var(--color-base-content) / 0.7; padding: 6px 0; }

  .btn-row.inline { display: flex; flex-wrap: wrap; gap: 8px; }
  .hidden-file { display: none; }
  .import-message { font-size: 12px; color: var(--color-primary); }
</style>
