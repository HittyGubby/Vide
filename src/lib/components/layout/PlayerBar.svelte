<script lang="ts">
  import {
    Play,
    SkipBack,
    SkipForward,
    Volume2,
    ListMusic,
  } from "lucide-svelte";
</script>

<footer class="playerbar">
  <!-- Left: Album art + song info -->
  <div class="player-left">
    <div class="album-art-placeholder">
      <span class="album-icon">
        <ListMusic size={20} />
      </span>
    </div>
    <div class="song-info">
      <span class="song-title truncate">未在播放</span>
      <span class="song-artist truncate">选择一首歌曲开始播放</span>
    </div>
  </div>

  <!-- Center: Controls (placeholder) -->
  <div class="player-center">
    <span class="placeholder-text">播放栏</span>
  </div>

  <!-- Right: Volume + extras (placeholder) -->
  <div class="player-right">
    <span class="volume-icon">
      <Volume2 size={16} />
    </span>
  </div>
</footer>

<style>
  .playerbar {
    display: flex;
    align-items: center;
    height: var(--playerbar-height);
    background: var(--bg-surface);
    border-top: 1px solid var(--border-subtle);
    padding: 0 12px;
    gap: 12px;
  }

  /* ── Left Section ────────────────────────────────── */
  .player-left {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
    flex: 1;
  }

  .album-art-placeholder {
    width: 44px;
    height: 44px;
    border-radius: 6px;
    background: var(--bg-elevated);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .album-icon {
    color: var(--text-tertiary);
  }

  .song-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .song-title {
    font-size: 13px;
    font-weight: 500;
    color: var(--text-primary);
  }

  .song-artist {
    font-size: 11px;
    color: var(--text-secondary);
  }

  /* ── Center Section ──────────────────────────────── */
  .player-center {
    display: flex;
    align-items: center;
    justify-content: center;
    flex: 1;
  }

  .placeholder-text {
    font-size: 11px;
    color: var(--text-tertiary);
  }

  /* ── Right Section ───────────────────────────────── */
  .player-right {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
    flex: 1;
  }

  .volume-icon {
    color: var(--text-secondary);
  }
</style>
