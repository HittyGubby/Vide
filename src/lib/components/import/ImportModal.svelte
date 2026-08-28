<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { Upload, Link, FileText, X, CheckCircle, AlertCircle, Loader } from "lucide-svelte";

  let { onclose }: { onclose: () => void } = $props();

  // Tab state
  type Tab = "file" | "url";
  let activeTab = $state<Tab>("file");

  // File import
  let filePath = $state("");
  let fileImporting = $state(false);
  let fileResult: { success: boolean; message: string } | null = null;

  // URL import (placeholder)
  let url = $state("");
  let urlImporting = $state(false);
  let urlResult: { success: boolean; message: string } | null = null;

  async function handleFileSelected(e: Event) {
    const input = e.target as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;

    // In Tauri 2, File objects from file inputs have a `path` property
    const path = (file as any).path || file.name;
    filePath = file.name;
    fileImporting = true;
    fileResult = null;

    try {
      const playlist = await invoke("import_m3u", { path: path as string });
      fileResult = {
        success: true,
        message: `成功导入: ${(playlist as any).name || file.name}`,
      };
    } catch (e) {
      fileResult = {
        success: false,
        message: `导入失败: ${e}`,
      };
    } finally {
      fileImporting = false;
    }
  }

  async function handleUrlImport() {
    const trimmed = url.trim();
    if (!trimmed) return;
    urlImporting = true;
    urlResult = null;

    try {
      const playlist = await invoke("import_playlist_url", { url: trimmed });
      const n = (playlist as any)?.entries?.length ?? 0;
      urlResult = {
        success: true,
        message: "已导入在线播放列表（" + n + " 首）",
      };
      url = "";
    } catch (e) {
      console.error("url import error:", e);
      urlResult = {
        success: false,
        message: String(e),
      };
    } finally {
      urlImporting = false;
    }
  }
</script>

<!-- Backdrop -->
<div class="backdrop" onclick={onclose} role="presentation"></div>

<div class="import-modal" role="dialog" aria-label="导入">
  <div class="modal-header">
    <span class="modal-title">导入</span>
    <button class="close-btn" onclick={onclose} aria-label="关闭">
      <X size={16} />
    </button>
  </div>

  <!-- Tabs -->
  <div class="tabs">
    <button class="tab" class:active={activeTab === "file"} onclick={() => (activeTab = "file")}>
      <FileText size={14} />
      <span>本地文件</span>
    </button>
    <button class="tab" class:active={activeTab === "url"} onclick={() => (activeTab = "url")}>
      <Link size={14} />
      <span>在线链接</span>
    </button>
  </div>

  <!-- File import tab -->
  {#if activeTab === "file"}
    <div class="tab-content">
      <p class="tab-desc">导入 M3U 播放列表文件。支持 .m3u 和 .m3u8 格式。</p>

      <div class="file-dropzone">
        <Upload size={24} class="dropzone-icon" />
        <p class="dropzone-text">选择或拖放 M3U 文件</p>
        <label class="file-btn">
          选择文件
          <input
            type="file"
            accept=".m3u,.m3u8,.txt"
            class="file-input"
            onchange={handleFileSelected}
          />
        </label>
        {#if filePath}
          <p class="selected-file">{filePath}</p>
        {/if}
      </div>

      {#if fileImporting}
        <div class="result status-loading">
          <Loader size={16} class="spin" />
          <span>正在导入...</span>
        </div>
      {/if}

      {#if fileResult}
        <div class="result" class:status-success={fileResult.success} class:status-error={!fileResult.success}>
          {#if fileResult.success}
            <CheckCircle size={16} />
          {:else}
            <AlertCircle size={16} />
          {/if}
          <span>{fileResult.message}</span>
        </div>
      {/if}
    </div>
  {/if}

  <!-- URL import tab -->
  {#if activeTab === "url"}
    <div class="tab-content">
      <p class="tab-desc">粘贴在线播放列表链接（支持 YouTube、Bilibili 等平台）。</p>

      <div class="url-input-group">
        <input
          type="url"
          class="url-input"
          placeholder="https://www.youtube.com/playlist?list=..."
          bind:value={url}
          onkeydown={(e) => { if (e.key === "Enter") handleUrlImport(); }}
        />
        <button class="import-btn" onclick={handleUrlImport} disabled={!url.trim() || urlImporting}>
          导入
        </button>
      </div>

      {#if urlImporting}
        <div class="result status-loading">
          <Loader size={16} class="spin" />
          <span>正在处理...</span>
        </div>
      {/if}

      {#if urlResult}
        <div class="result" class:status-success={urlResult.success} class:status-error={!urlResult.success}>
          {#if urlResult.success}
            <CheckCircle size={16} />
          {:else}
            <AlertCircle size={16} />
          {/if}
          <span>{urlResult.message}</span>
        </div>
      {/if}
    </div>
  {/if}

  <div class="modal-footer">
    <span class="hint">支持格式: M3U, M3U8</span>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 500;
    background: rgba(0, 0, 0, 0.4);
  }

  .import-modal {
    position: fixed;
    z-index: 501;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: 420px;
    max-height: 480px;
    background: var(--bg-elevated);
    border: 1px solid var(--border-normal);
    border-radius: 10px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 16px 10px;
  }

  .modal-title {
    font-size: 15px;
    font-weight: 600;
    color: var(--text-primary);
  }

  .close-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border-radius: 6px;
    color: var(--text-secondary);
    background: none;
    border: none;
  }

  .close-btn:hover { background: var(--bg-hover); color: var(--text-primary); }

  /* ── Tabs ───────────────────────────────────────── */
  .tabs {
    display: flex;
    gap: 0;
    padding: 0 16px;
    border-bottom: 1px solid var(--border-subtle);
  }

  .tab {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 14px;
    border-bottom: 2px solid transparent;
    color: var(--text-secondary);
    font-size: 12px;
    background: none;
    border-top: none;
    border-left: none;
    border-right: none;
    transition: color 0.15s, border-color 0.15s;
  }

  .tab:hover { color: var(--text-primary); }
  .tab.active {
    color: var(--accent-primary);
    border-bottom-color: var(--accent-primary);
  }

  /* ── Content ─────────────────────────────────────── */
  .tab-content {
    flex: 1;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    overflow-y: auto;
  }

  .tab-desc {
    font-size: 12px;
    color: var(--text-tertiary);
    line-height: 1.4;
  }

  /* ── File Dropzone ───────────────────────────────── */
  .file-dropzone {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 24px 16px;
    border: 2px dashed var(--border-normal);
    border-radius: 8px;
    text-align: center;
  }

  .dropzone-icon { color: var(--text-tertiary); }
  .dropzone-text { font-size: 13px; color: var(--text-secondary); }

  .file-btn {
    display: inline-block;
    padding: 6px 16px;
    border-radius: 6px;
    background: var(--accent-primary);
    color: white;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition: background 0.15s;
  }

  .file-btn:hover { background: var(--accent-hover); }
  .file-input { display: none; }

  .selected-file {
    font-size: 11px;
    color: var(--text-tertiary);
    margin-top: 4px;
  }

  /* ── URL Input ───────────────────────────────────── */
  .url-input-group {
    display: flex;
    gap: 8px;
  }

  .url-input {
    flex: 1;
    padding: 8px 10px;
    background: var(--bg-input);
    border: 1px solid var(--border-subtle);
    border-radius: 6px;
    color: var(--text-primary);
    font-size: 12.5px;
    outline: none;
    user-select: text;
  }

  .url-input:focus { border-color: var(--accent-primary); }
  .url-input::placeholder { color: var(--text-tertiary); }

  .import-btn {
    padding: 8px 16px;
    border-radius: 6px;
    background: var(--accent-primary);
    color: white;
    font-size: 12px;
    font-weight: 500;
    border: none;
    white-space: nowrap;
  }

  .import-btn:hover:not(:disabled) { background: var(--accent-hover); }
  .import-btn:disabled { opacity: 0.4; cursor: default; }

  /* ── Results ──────────────────────────────────────── */
  .result {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    border-radius: 6px;
    font-size: 12px;
  }

  .status-success { background: rgba(76, 175, 80, 0.15); color: var(--status-success); }
  .status-error { background: rgba(244, 67, 54, 0.15); color: var(--status-error); }
  .status-loading { background: var(--bg-surface); color: var(--text-secondary); }

  .spin { animation: spin 1s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }

  /* ── Footer ──────────────────────────────────────── */
  .modal-footer {
    padding: 8px 16px;
    border-top: 1px solid var(--border-subtle);
  }

  .hint {
    font-size: 10px;
    color: var(--text-tertiary);
  }
</style>
