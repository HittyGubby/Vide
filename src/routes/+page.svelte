<script lang="ts">
  import { Search, ArrowUpDown, Tag } from "lucide-svelte";

  const columns = [
    { id: "title", label: "标题", width: "2fr" },
    { id: "artist", label: "歌手", width: "1.5fr" },
    { id: "album", label: "专辑", width: "1.5fr" },
    { id: "tags", label: "标签", width: "1.5fr" },
    { id: "duration", label: "时长", width: "70px" },
  ];
</script>

<div class="library-view">
  <!-- Local search bar -->
  <div class="search-bar">
    <div class="search-input-wrapper">
      <span class="search-icon">
        <Search size={14} />
      </span>
      <input
        type="text"
        class="local-search"
        placeholder="搜索我的音乐... (留空 = 我的库)"
      />
    </div>
    <button class="sort-btn" title="排序">
      <ArrowUpDown size={14} />
      <span class="sort-label">排序</span>
    </button>
  </div>

  <!-- Song table header -->
  <div class="table-header" role="row">
    <div class="col col-index">#</div>
    {#each columns as col}
      <div
        class="col"
        style="flex: {col.width}"
        role="columnheader"
        title="{col.label} — 点击排序"
      >
        <span>{col.label}</span>
      </div>
    {/each}
  </div>

  <!-- Empty state -->
  <div class="empty-state">
    <div class="empty-icon">
      <Tag size={32} />
    </div>
    <p class="empty-title">还没有歌曲</p>
    <p class="empty-desc">添加库目录或导入播放列表开始使用</p>
  </div>
</div>

<style>
  .library-view {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  /* ── Search Bar ──────────────────────────────────── */
  .search-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 16px;
    border-bottom: 1px solid var(--border-subtle);
  }

  .search-input-wrapper {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: 1;
    height: 30px;
    padding: 0 10px;
    background: var(--bg-input);
    border: 1px solid var(--border-subtle);
    border-radius: 6px;
    transition: border-color 0.15s;
  }

  .search-input-wrapper:focus-within {
    border-color: var(--accent-primary);
  }

  .search-icon {
    color: var(--text-tertiary);
    flex-shrink: 0;
  }

  .local-search {
    flex: 1;
    height: 100%;
    background: none;
    border: none;
    color: var(--text-primary);
    font-size: 13px;
    outline: none;
    user-select: text;
  }

  .local-search::placeholder {
    color: var(--text-tertiary);
  }

  .sort-btn {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 6px 10px;
    border-radius: 6px;
    color: var(--text-secondary);
    background: var(--bg-input);
    border: 1px solid var(--border-subtle);
    font-size: 12px;
    white-space: nowrap;
    transition:
      background 0.15s,
      color 0.15s;
  }

  .sort-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .sort-label {
    display: none;
  }

  /* ── Table Header ────────────────────────────────── */
  .table-header {
    display: flex;
    align-items: center;
    height: 32px;
    padding: 0 12px 0 8px;
    background: var(--bg-surface);
    border-bottom: 1px solid var(--border-subtle);
    font-size: 11px;
    font-weight: 500;
    color: var(--text-tertiary);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .col {
    display: flex;
    align-items: center;
    gap: 4px;
    cursor: pointer;
    user-select: none;
    min-width: 0;
  }

  .col:hover {
    color: var(--text-secondary);
  }

  .col-index {
    width: 36px;
    flex: none;
    text-align: center;
  }

  /* ── Empty State ─────────────────────────────────── */
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
