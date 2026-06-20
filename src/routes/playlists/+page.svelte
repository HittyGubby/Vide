<script lang="ts">
  import { ListMusic } from 'lucide-svelte';
</script>

<div class="playlists-view">
  <div class="empty-state">
    <div class="empty-icon">
      <ListMusic size={32} />
    </div>
    <p class="empty-title">还没有播放列表</p>
    <p class="empty-desc">导入 M3U 文件或在搜索结果中创建列表</p>
  </div>
</div>

<style>
  .playlists-view {
    display: flex;
    flex-direction: column;
    height: 100%;
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
    max-width: 240px;
    line-height: 1.5;
  }
</style>
