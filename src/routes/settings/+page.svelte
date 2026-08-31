<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { Settings as SettingsIcon, Scan, Download, Upload, FolderPlus, Edit, X, Check } from "lucide-svelte";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import ShortcutCell from "$lib/components/settings/ShortcutCell.svelte";
  import MonacoCssEditor from "$lib/components/settings/MonacoCssEditor.svelte";
  import { SHORTCUT_ACTIONS } from "$lib/shortcuts";
  import type { AppSettings, CacheStat, ShortcutAction } from "$lib/types";

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
  let cacheStats = $state<CacheStat[]>([]);
  let cacheBusy = $state<string | null>(null);
  let cacheNote = $state<string | null>(null);

  onMount(async () => {
    await settingsStore.load();
    try {
      roots = await invoke("get_library_roots");
      excludeRoots = await invoke("get_exclude_roots");
    } catch {
      roots = [];
      excludeRoots = [];
    }
    await loadCacheStats();
  });

  // ── Cache manager ──────────────────────────
  async function loadCacheStats() {
    try {
      cacheStats = await invoke<CacheStat[]>("get_cache_stats");
    } catch (e) {
      console.error("load cache stats error:", e);
    }
  }

  async function clearCache(kind: string) {
    cacheBusy = kind;
    cacheNote = null;
    try {
      const removed = await invoke<number>("clear_cache_kind", { kind });
      cacheNote = "已清除 " + removed + " 项";
      await loadCacheStats();
    } catch (e) {
      console.error("clear cache error:", e);
      cacheNote = "清除失败";
    } finally {
      cacheBusy = null;
    }
  }

  function formatBytes(bytes?: number | null): string {
    if (bytes == null) return "";
    const units = ["B", "KB", "MB", "GB", "TB"];
    let v = bytes;
    let i = 0;
    while (v >= 1024 && i < units.length - 1) {
      v /= 1024;
      i++;
    }
    const digits = i === 0 || v >= 100 ? 0 : v >= 10 ? 1 : 2;
    return v.toFixed(digits) + units[i];
  }

  const positionOptions = [
    { id: "top", label: "顶部" },
    { id: "center", label: "中部" },
    { id: "bottom", label: "下部" },
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

  async function setFloat(key: keyof AppSettings, value: string) {
    const n = parseFloat(value);
    if (isNaN(n)) return;
    await settingsStore.update(key, n);
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
        <span class="row-label">主题色</span>
        <div class="color-row">
          <input
            type="color"
            value={settingsStore.settings.theme_primary}
            oninput={(e) => setText("theme_primary", (e.currentTarget as HTMLInputElement).value)}
            class="color-input"
          />
          <input
            type="text"
            value={settingsStore.settings.theme_primary}
            onchange={(e) => setText("theme_primary", (e.currentTarget as HTMLInputElement).value)}
            placeholder="#5f5dfd"
            class="text-input color-text"
          />
          <button
            class="clear-btn"
            onclick={() => setText("theme_primary", "#5f5dfd")}
          >
            重置
          </button>
        </div>
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

  <!-- 快捷键 -->
  <section class="group">
    <h2 class="group-title">快捷键</h2>
    <div class="setting-card">
      <table class="shortcut-table">
        <thead>
          <tr>
            <th class="sc-action">操作</th>
            <th class="sc-key">应用内快捷键</th>
            <th class="sc-key">全局快捷键</th>
          </tr>
        </thead>
        <tbody>
          {#each SHORTCUT_ACTIONS as item}
            <tr>
              <td class="sc-action">{item.label}</td>
              <td class="sc-key">
                <ShortcutCell
                  value={settingsStore.settings.shortcuts[item.id].in_app}
                  onchange={(v) => updateShortcut(item.id, "in_app", v)}
                />
              </td>
              <td class="sc-key">
                <ShortcutCell
                  value={settingsStore.settings.shortcuts[item.id].global}
                  onchange={(v) => updateShortcut(item.id, "global", v)}
                />
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
      <p class="sc-hint">
        点击单元格后按下新的组合键即可修改；Backspace 清除，Esc 取消。全局快捷键在应用未获得焦点时同样生效。
      </p>
      <div class="row input-row">
        <span class="row-label">快进/快退时长（秒）</span>
        <input
          type="number"
          min="1"
          max="120"
          step="1"
          value={settingsStore.settings.seek_step_secs}
          onchange={(e) => setFloat("seek_step_secs", (e.currentTarget as HTMLInputElement).value)}
          class="num-input"
        />
      </div>
      <div class="row input-row">
        <span class="row-label">音量调节步长</span>
        <input
          type="number"
          min="1"
          max="50"
          step="1"
          value={settingsStore.settings.volume_step}
          onchange={(e) => setNumber("volume_step", (e.currentTarget as HTMLInputElement).value)}
          class="num-input"
        />
      </div>
    </div>
  </section>

  <!-- 缓存管理 -->
  <section class="group">
    <h2 class="group-title">缓存管理</h2>
    <div class="setting-card">
      <table class="cache-table">
        <thead>
          <tr>
            <th>缓存项</th>
            <th class="cache-amount">数量（大小）</th>
            <th class="cache-action">操作</th>
          </tr>
        </thead>
        <tbody>
          {#each cacheStats as stat}
            <tr>
              <td>{stat.label}</td>
              <td class="cache-amount">
                {stat.count}{stat.size != null ? "（" + formatBytes(stat.size) + "）" : ""}
              </td>
              <td class="cache-action">
                <button
                  class="clear-btn"
                  disabled={cacheBusy === stat.kind}
                  onclick={() => clearCache(stat.kind)}
                >
                  {cacheBusy === stat.kind ? "清除中…" : "清除"}
                </button>
              </td>
            </tr>
          {:else}
            <tr><td class="cache-empty" colspan="3">加载中…</td></tr>
          {/each}
        </tbody>
      </table>
      {#if cacheNote}
        <p class="sc-hint">{cacheNote}</p>
      {/if}
    </div>
  </section>

  <!-- 桌面歌词 -->
  <section class="group">
    <h2 class="group-title">桌面歌词</h2>
    <div class="setting-card">
      <label class="row checkbox-row">
        <input
          type="checkbox"
          checked={settingsStore.settings.desktop_lyrics_enabled}
          onchange={(e) => toggle("desktop_lyrics_enabled", (e.currentTarget as HTMLInputElement).checked)}
        />
        <span>展示桌面歌词窗口</span>
      </label>
      <label class="row checkbox-row">
        <input
          type="checkbox"
          checked={settingsStore.settings.desktop_lyrics_always_on_top}
          onchange={(e) => toggle("desktop_lyrics_always_on_top", (e.currentTarget as HTMLInputElement).checked)}
        />
        <span>保持歌词窗口在最顶端</span>
      </label>
      <label class="row checkbox-row">
        <input
          type="checkbox"
          checked={settingsStore.settings.desktop_lyrics_locked}
          onchange={(e) => toggle("desktop_lyrics_locked", (e.currentTarget as HTMLInputElement).checked)}
        />
        <span>锁定歌词窗口</span>
      </label>
      <p class="sc-hint">
        锁定后窗口不再响应拖拽与缩放，并且所有输入事件都会透穿到下层的窗口。
      </p>
      <div class="row radio-row">
        <span class="row-label">正在播放的歌词行位置</span>
        <div class="radio-group">
          {#each positionOptions as opt}
            <label class="radio-opt">
              <input
                type="radio"
                name="desktop_lyrics_position"
                value={opt.id}
                checked={settingsStore.settings.desktop_lyrics_position === opt.id}
                onchange={() => setText("desktop_lyrics_position", opt.id)}
              />
              <span>{opt.label}</span>
            </label>
          {/each}
        </div>
      </div>
    </div>
  </section>

  <!-- 歌词样式 -->
  <section class="group">
    <h2 class="group-title">歌词样式</h2>
    <div class="setting-card">
      <p class="sc-hint">
        两段 CSS 分别作用于悬浮歌词窗口和播放详情页面的歌词区域，保存后立即生效。自动滚动由上面的“歌词行位置”控制，其余外观完全交给这里的样式。
      </p>
      <p class="editor-label">悬浮歌词样式</p>
      <MonacoCssEditor
        value={settingsStore.settings.desktop_lyrics_css}
        onchange={(next) => setText("desktop_lyrics_css", next)}
        heightPx={300}
      />
      <p class="editor-label">播放详情歌词样式</p>
      <MonacoCssEditor
        value={settingsStore.settings.detail_lyrics_css}
        onchange={(next) => setText("detail_lyrics_css", next)}
        heightPx={300}
      />
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

  .shortcut-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
  }
  .shortcut-table th {
    text-align: left;
    font-size: 11px;
    font-weight: 600;
    color: var(--color-base-content) / 0.55;
    padding: 8px 6px;
    border-bottom: 1px solid var(--panel-border);
  }
  .shortcut-table th.sc-key,
  .shortcut-table td.sc-key { text-align: center; }
  .shortcut-table td {
    padding: 4px 6px;
    border-bottom: 1px solid var(--panel-border);
    vertical-align: middle;
  }
  .shortcut-table tr:last-child td { border-bottom: none; }
  .shortcut-table td.sc-action {
    color: var(--color-base-content);
    white-space: nowrap;
  }
  .shortcut-table td.sc-key { width: 34%; }

  .sc-hint {
    font-size: 11px;
    color: var(--color-base-content) / 0.55;
    line-height: 1.6;
    padding: 6px 0 2px;
  }

  .color-row { display: flex; align-items: center; gap: 8px; }
  .color-input {
    width: 40px;
    height: 26px;
    padding: 0;
    border: 1px solid var(--panel-border);
    border-radius: 6px;
    background: none;
    cursor: pointer;
  }
  .color-text { width: 110px; }

  .editor-label {
    color: var(--color-base-content) / 0.7;
    padding: 8px 2px 2px;
  }

  .cache-table { width: 100%; border-collapse: collapse; font-size: 13px; }
  .cache-table th {
    text-align: left;
    font-size: 11px;
    font-weight: 600;
    color: var(--color-base-content) / 0.55;
    padding: 8px 6px;
    border-bottom: 1px solid var(--panel-border);
  }
  .cache-table th.cache-amount { text-align: right; }
  .cache-table th.cache-action { text-align: right; }
  .cache-table td {
    padding: 6px;
    border-bottom: 1px solid var(--panel-border);
    vertical-align: middle;
  }
  .cache-table tr:last-child td { border-bottom: none; }
  .cache-amount { text-align: right; font-variant-numeric: tabular-nums; white-space: nowrap; }
  .cache-action { text-align: right; width: 90px; }
  .cache-empty { text-align: center; color: var(--color-base-content) / 0.5; }

  .clear-btn {
    padding: 4px 12px;
    border: 1px solid var(--panel-border-strong, var(--panel-border));
    border-radius: 6px;
    background: var(--color-base-100);
    color: var(--color-base-content);
    font: inherit;
    font-size: 12px;
    cursor: pointer;
    transition: border-color 0.12s, color 0.12s;
  }
  .clear-btn:hover:not(:disabled) { border-color: var(--color-primary); color: var(--color-primary); }
  .clear-btn:disabled { opacity: 0.5; cursor: default; }

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
