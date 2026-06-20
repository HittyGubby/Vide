<script lang="ts">
  import { ListMusic, Upload } from "lucide-svelte";
  import ImportModal from "$lib/components/import/ImportModal.svelte";

  let showImport = $state(false);
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

  <div class="empty-state">
    <div class="empty-icon">
      <ListMusic size={32} />
    </div>
    <p class="empty-title">还没有播放列表</p>
    <p class="empty-desc">点击上方"导入"按钮添加 M3U 播放列表</p>
  </div>
</div>

<style>
  .playlists-view {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  .toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 16px;
    border-bottom: 1px solid var(--border-subtle);
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
