<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { ListMusic, Upload, FileText } from "lucide-svelte";
  import ImportModal from "$lib/components/import/ImportModal.svelte";
  import type { Playlist } from "$lib/types";

  let showImport = $state(false);
  let playlists = $state<Playlist[]>([]);
  let loading = $state(true);

  onMount(async () => {
    try {
      playlists = await invoke("list_playlists");
    } catch {
      playlists = [];
    } finally {
      loading = false;
    }
  });
</script>

<div class="playlists-view">
  {#if showImport}
    <ImportModal onclose={() => (showImport = false)} />
  {/if}

  <div class="toolbar">
    <span class="page-title">播放列表</span>
    <button class="import-btn" onclick={() => (showImport = true)}>
      <Upload size={14} />
      <span>导入</span>
    </button>
  </div>

  {#if loading}
    <div class="empty-state">
      <p class="empty-title">加载中...</p>
    </div>
  {:else if playlists.length === 0}
    <div class="empty-state">
      <div class="empty-icon">
        <ListMusic size={32} />
      </div>
      <p class="empty-title">还没有播放列表</p>
      <p class="empty-desc">点击上方"导入"按钮添加 M3U 播放列表</p>
    </div>
  {:else}
    <div class="playlist-list">
      {#each playlists as pl}
        <div class="playlist-card">
          <FileText size={16} class="pl-icon" />
          <div class="pl-info">
            <span class="pl-name truncate">{pl.name}</span>
            <span class="pl-meta">
              {pl.entries?.length ?? 0} 首曲目
              {#if pl.source?.file_path}
                · {pl.source.file_path}
              {/if}
            </span>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .playlists-view {
    display: flex;
    flex-direction: column;
    height: 100%;
    user-select: none;
  }

  .toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 16px;
    border-bottom: 1px solid var(--border-subtle);
    user-select: none;
  }

  .page-title {
    font-size: 14px;
    font-weight: 600;
    color: var(--text-primary);
  }

  .import-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 14px;
    border-radius: 6px;
    background: var(--accent-primary);
    color: white;
    font-size: 12px;
    font-weight: 500;
    border: none;
    transition: background 0.15s;
  }

  .import-btn:hover {
    background: var(--accent-hover);
  }

  /* ── Playlist List ────────────────────────────── */
  .playlist-list {
    flex: 1;
    overflow-y: auto;
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .playlist-card {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    border-radius: 6px;
    cursor: pointer;
    transition: background 0.1s;
  }

  .playlist-card:hover {
    background: var(--bg-hover);
  }

  .pl-icon {
    color: var(--accent-primary);
    flex-shrink: 0;
  }

  .pl-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    flex: 1;
  }

  .pl-name {
    font-size: 13px;
    font-weight: 500;
    color: var(--text-primary);
  }

  .pl-meta {
    font-size: 11px;
    color: var(--text-tertiary);
  }

  /* ── Empty State ───────────────────────────────── */
  .empty-state {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 40px;
    color: var(--text-tertiary);
  }

  .empty-icon {
    opacity: 0.4;
    margin-bottom: 4px;
  }
  .empty-title {
    font-size: 15px;
    font-weight: 500;
    color: var(--text-secondary);
  }
  .empty-desc {
    font-size: 12px;
    text-align: center;
    max-width: 280px;
    line-height: 1.5;
  }
</style>
