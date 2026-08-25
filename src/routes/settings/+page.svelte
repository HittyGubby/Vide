<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { Settings, Scan, Download, FolderPlus } from "lucide-svelte";

  let roots = $state<string[]>([]);
  let newRootPath = $state("");
  let showInput = $state(false);
  let scanning = $state(false);
  let scanResult: {
    scanned: number;
    added: number;
    skipped: number;
    errors: string[];
  } | null = null;
  let exporting = $state(false);
  let exportDone = $state(false);

  onMount(async () => {
    try {
      roots = await invoke("get_library_roots");
    } catch {
      roots = [];
    }
  });

  async function addRoot() {
    const path = newRootPath.trim();
    if (!path) return;
    try {
      await invoke("add_library_root", { path });
      roots = [...roots, path];
      newRootPath = "";
      showInput = false;
    } catch (e) {
      console.error("add root error:", e);
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

  async function exportAllSongs() {
    exporting = true;
    exportDone = false;
    try {
      const songs = await invoke("list_songs");
      const json = JSON.stringify(songs, null, 2);
      const blob = new Blob([json], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `vide_library_${new Date().toISOString().slice(0, 10)}.json`;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
      exportDone = true;
    } catch (e) {
      console.error("export error:", e);
    } finally {
      exporting = false;
    }
  }
</script>

<div class="settings-view">
  <div class="settings-header">
    <Settings size={18} />
    <h1>设置</h1>
  </div>

  <!-- Library Roots -->
  <section class="setting-group">
    <h2 class="group-title">库管理</h2>
    <p class="group-desc">添加音乐文件夹后扫描，歌曲会自动加入"我的库"</p>
    <div class="setting-card">
      <div class="card-row">
        <span class="card-label">库文件夹</span>
        <button class="btn-secondary" onclick={() => (showInput = !showInput)}>
          <FolderPlus size={13} />
          <span>添加</span>
        </button>
      </div>
      {#if showInput}
        <div class="add-root-row">
          <input
            type="text"
            class="root-input"
            placeholder="/path/to/music"
            bind:value={newRootPath}
            onkeydown={(e) => {
              if (e.key === "Enter") addRoot();
            }}
          />
          <button
            class="btn-add"
            onclick={addRoot}
            disabled={!newRootPath.trim()}>确定</button
          >
        </div>
      {/if}
      <div class="root-list">
        {#if roots.length === 0}
          <div class="root-empty">暂未添加库目录</div>
        {:else}
          {#each roots as root}
            <div class="root-item">{root}</div>
          {/each}
        {/if}
      </div>
    </div>
    <button
      class="btn-primary"
      onclick={scanLibrary}
      disabled={scanning || roots.length === 0}
    >
      <Scan size={14} />
      <span>{scanning ? "扫描中..." : "立即扫描"}</span>
    </button>
    {#if scanResult}
      <div class="scan-result">
        扫描完成: {scanResult.scanned} 个文件, 新增 {scanResult.added} 首, 跳过 {scanResult.skipped}
        首
        {#if scanResult.errors.length > 0}
          · {scanResult.errors.length} 个错误
        {/if}
      </div>
    {/if}
  </section>

  <!-- Data Export -->
  <section class="setting-group">
    <h2 class="group-title">数据导出</h2>
    <p class="group-desc">导出全部歌曲为 JSON（包含所有元信息和标签）</p>
    <button class="btn-primary" onclick={exportAllSongs} disabled={exporting}>
      <Download size={14} />
      <span>{exporting ? "导出中..." : "导出全部歌曲"}</span>
    </button>
    {#if exportDone}
      <span class="export-hint">✓ 下载已开始</span>
    {/if}
  </section>

  <!-- About -->
  <section class="setting-group">
    <h2 class="group-title">关于</h2>
    <div class="setting-card">
      <div class="info-row">
        <span class="info-label">版本</span><span class="info-value">0.1.0</span
        >
      </div>
      <div class="info-row">
        <span class="info-label">后端</span><span class="info-value status-ok"
          >运行中</span
        >
      </div>
    </div>
  </section>
</div>

<style>
  .settings-view {
    max-width: 640px;
    padding: 24px 32px;
    display: flex;
    flex-direction: column;
    gap: 28px;
    user-select: none;
  }

  .settings-header {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text-primary);
  }

  .settings-header h1 {
    font-size: 18px;
    font-weight: 600;
  }

  .setting-group {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .group-title {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary);
  }

  .group-desc {
    font-size: 11px;
    color: var(--text-tertiary);
    margin-bottom: 4px;
  }

  .setting-card {
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: 8px;
    padding: 12px 16px;
  }

  .card-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .card-label {
    font-size: 13px;
    color: var(--text-primary);
  }

  .add-root-row {
    display: flex;
    gap: 6px;
    margin-top: 8px;
  }

  .root-input {
    flex: 1;
    padding: 5px 8px;
    border-radius: 5px;
    background: var(--bg-input);
    border: 1px solid var(--border-subtle);
    color: var(--text-primary);
    font-size: 12px;
    outline: none;
    user-select: text;
  }

  .root-input:focus {
    border-color: var(--accent-primary);
  }

  .btn-add {
    padding: 5px 12px;
    border-radius: 5px;
    background: var(--accent-primary);
    color: white;
    font-size: 11px;
    font-weight: 500;
    border: none;
    white-space: nowrap;
  }

  .btn-add:hover:not(:disabled) {
    background: var(--accent-hover);
  }
  .btn-add:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .root-list {
    margin-top: 8px;
  }

  .root-empty {
    font-size: 12px;
    color: var(--text-tertiary);
    padding: 4px 0;
  }

  .root-item {
    font-size: 12px;
    color: var(--text-primary);
    padding: 4px 0;
    font-family: monospace;
  }

  .btn-secondary {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 4px 12px;
    border-radius: 6px;
    background: var(--bg-elevated);
    border: 1px solid var(--border-normal);
    color: var(--text-secondary);
    font-size: 12px;
    transition: background 0.15s;
  }

  .btn-secondary:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .btn-primary {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 8px 16px;
    border-radius: 6px;
    background: var(--accent-primary);
    color: white;
    font-size: 12px;
    font-weight: 500;
    align-self: flex-start;
    border: none;
    transition: background 0.15s;
  }

  .btn-primary:hover:not(:disabled) {
    background: var(--accent-hover);
  }
  .btn-primary:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .scan-result {
    font-size: 11px;
    color: var(--text-secondary);
    padding: 6px 0;
  }

  .export-hint {
    font-size: 11px;
    color: var(--status-success);
  }

  .info-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 4px 0;
  }

  .info-label {
    font-size: 12px;
    color: var(--text-secondary);
  }
  .info-value {
    font-size: 12px;
    color: var(--text-primary);
  }
  .status-ok {
    color: var(--status-success);
  }
</style>
