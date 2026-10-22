<script lang="ts">
  import { invoke } from "$lib/transport";
  import { X } from "lucide-svelte";
  import type { Song } from "$lib/types";

  let {
    song,
    onclose,
    onsaved,
  }: {
    song: Song;
    onclose: () => void;
    onsaved: (song: Song) => void;
  } = $props();

  // Prefill with the current effective values so the user can edit rather than
  // retype the whole name; clearing a field falls back to the raw metadata.
  let title = $state(song.metadata.display_title ?? song.title);
  let artist = $state(song.metadata.artist_override ?? song.artist);
  let album = $state(song.metadata.album_override ?? song.album);
  let notes = $state(song.metadata.notes ?? "");
  let saving = $state(false);
  let error = $state<string | null>(null);

  async function save() {
    saving = true;
    error = null;
    try {
      const updated = await invoke<Song>("update_song_metadata", {
        id: song.id,
        title: title.trim() || null,
        artist: artist.trim() || null,
        album: album.trim() || null,
        notes: notes.trim() || null,
      });
      onsaved(updated);
      onclose();
    } catch (e) {
      error = String(e);
      console.error("update song metadata error:", e);
    } finally {
      saving = false;
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="backdrop" onclick={onclose} role="presentation"></div>

<div class="meta-editor" role="dialog" aria-label="编辑歌曲信息">
  <div class="editor-header">
    <span class="editor-title">编辑歌曲信息</span>
    <span class="editor-raw">{song.title} · {song.artist}</span>
    <button class="close-btn" onclick={onclose} aria-label="关闭"><X size={14} /></button>
  </div>

  <div class="editor-body">
    <label class="field">
      <span>标题</span>
      <input type="text" bind:value={title} placeholder={song.title} />
    </label>
    <label class="field">
      <span>艺术家</span>
      <input type="text" bind:value={artist} placeholder={song.artist} />
    </label>
    <label class="field">
      <span>专辑</span>
      <input type="text" bind:value={album} placeholder={song.album} />
    </label>
    <label class="field">
      <span>笔记</span>
      <textarea bind:value={notes} rows="3" placeholder="可选"></textarea>
    </label>
    <p class="hint">留空时使用文件/来源的原始信息。原始值不会被覆盖，重新扫描也不会丢失这里的修改。</p>
    {#if error}
      <p class="error">{error}</p>
    {/if}
  </div>

  <div class="editor-footer">
    <button class="btn-cancel" onclick={onclose}>取消</button>
    <button class="btn-save" onclick={save} disabled={saving}>
      {saving ? "保存中…" : "保存"}
    </button>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 500;
    background: rgba(0, 0, 0, 0.45);
  }

  .meta-editor {
    position: fixed;
    z-index: 501;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: 380px;
    max-width: 92vw;
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    background: var(--bg-elevated);
    border: 1px solid var(--border-normal);
    border-radius: 10px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
    overflow: hidden;
  }

  .editor-header {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 12px 14px 8px;
    border-bottom: 1px solid var(--border-subtle);
  }
  .editor-title { font-size: 13px; font-weight: 600; color: var(--text-primary); }
  .editor-raw {
    padding-right: 24px;
    font-size: 11px;
    color: var(--text-tertiary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .close-btn {
    position: absolute;
    top: 10px;
    right: 10px;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border-radius: 5px;
    background: none;
    border: none;
    color: var(--text-tertiary);
  }
  .close-btn:hover { background: var(--bg-surface); color: var(--text-primary); }

  .editor-body {
    display: flex;
    flex-direction: column;
    gap: 9px;
    padding: 12px 14px;
    overflow-y: auto;
  }

  .field { display: flex; flex-direction: column; gap: 4px; }
  .field span { font-size: 11px; color: var(--text-tertiary); }
  .field input,
  .field textarea {
    width: 100%;
    padding: 6px 8px;
    border-radius: 6px;
    background: var(--bg-input);
    border: 1px solid var(--border-subtle);
    color: var(--text-primary);
    font-size: 12.5px;
    outline: none;
    user-select: text;
    resize: vertical;
  }
  .field input:focus,
  .field textarea:focus { border-color: var(--accent-primary); }

  .hint { font-size: 10.5px; line-height: 1.5; color: var(--text-tertiary); }
  .error { font-size: 11px; color: var(--color-error); }

  .editor-footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 10px 14px;
    border-top: 1px solid var(--border-subtle);
  }
  .btn-cancel,
  .btn-save {
    padding: 6px 14px;
    border-radius: 6px;
    border: none;
    font-size: 12px;
  }
  .btn-cancel { background: var(--bg-surface); color: var(--text-secondary); }
  .btn-cancel:hover { background: var(--bg-base); }
  .btn-save { background: var(--color-primary); color: var(--color-primary-content); }
  .btn-save:disabled { opacity: 0.5; }
</style>
