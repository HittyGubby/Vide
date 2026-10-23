<script lang="ts">
  import { onMount } from "svelte";
  import { X } from "lucide-svelte";
  import { invoke } from "$lib/transport";
  import type { Song } from "$lib/types";

  let { songs, onclose, onsaved }: {
    songs: Song[];
    onclose: () => void;
    onsaved: (songs: Song[]) => void;
  } = $props();

  function titleOf(song: Song) {
    return song.metadata.display_title ?? song.title;
  }
  function artistOf(song: Song) {
    return song.metadata.artist_override ?? song.artist;
  }
  function albumOf(song: Song) {
    return song.metadata.album_override ?? song.album;
  }

  let titles = $state(songs.map(titleOf).join("\n"));
  let artists = $state(songs.map(artistOf).join("\n"));
  let albums = $state(songs.map(albumOf).join("\n"));
  let notes = $state(songs.map((song) => song.metadata.notes ?? "").join("\n"));
  let busy = $state(false);
  let error = $state<string | null>(null);

  function lineAt(value: string, index: number): string | null {
    const line = value.split("\n")[index] ?? "";
    return line.trim() ? line.trim() : null;
  }

  async function save() {
    busy = true;
    error = null;
    try {
      const updated: Song[] = [];
      for (let i = 0; i < songs.length; i += 1) {
        updated.push(
          await invoke<Song>("update_song_metadata", {
            id: songs[i].id,
            title: lineAt(titles, i),
            artist: lineAt(artists, i),
            album: lineAt(albums, i),
            notes: lineAt(notes, i),
          }),
        );
      }
      onsaved(updated);
      onclose();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<div class="backdrop" role="presentation" onclick={() => !busy && onclose()}>
  <div class="modal" role="dialog" aria-label="批量编辑信息" onclick={(e) => e.stopPropagation()}>
    <div class="head">
      <h2>批量编辑信息 · {songs.length} 首</h2>
      <button class="icon" onclick={onclose} disabled={busy}><X size={16} /></button>
    </div>
    <p class="hint">每个输入框一行对应一首歌，顺序与列表一致。留空表示清除该字段。</p>
    <label>标题<textarea bind:value={titles} rows={Math.max(2, songs.length)}></textarea></label>
    <label>作者<textarea bind:value={artists} rows={Math.max(2, songs.length)}></textarea></label>
    <label>专辑<textarea bind:value={albums} rows={Math.max(2, songs.length)}></textarea></label>
    <label>备注<textarea bind:value={notes} rows={Math.max(2, songs.length)}></textarea></label>
    {#if error}<p class="error">{error}</p>{/if}
    <div class="actions">
      <button onclick={onclose} disabled={busy}>取消</button>
      <button class="primary" onclick={() => void save()} disabled={busy}>{busy ? "保存中…" : "保存"}</button>
    </div>
  </div>
</div>

<style>
  .backdrop { position: fixed; inset: 0; z-index: 950; background: rgba(0,0,0,.55); display: flex; align-items: center; justify-content: center; }
  .modal { width: min(560px, calc(100vw - 32px)); max-height: calc(100vh - 48px); overflow: auto; display: flex; flex-direction: column; gap: 8px; padding: 16px; border-radius: 10px; background: var(--color-base-200); border: 1px solid var(--panel-border); }
  .head { display: flex; align-items: center; justify-content: space-between; }
  h2 { margin: 0; font-size: 15px; }
  .hint { margin: 0; font-size: 11px; color: var(--color-base-content) / 0.55; }
  label { display: flex; flex-direction: column; gap: 4px; font-size: 11px; color: var(--color-base-content) / 0.7; }
  textarea { resize: vertical; min-height: 44px; padding: 6px 8px; border: 1px solid var(--panel-border); border-radius: 5px; background: var(--color-base-100); color: inherit; font: inherit; font-size: 12px; }
  textarea:focus { outline: none; border-color: var(--color-primary); }
  .icon { border: 0; background: none; color: inherit; cursor: pointer; }
  .actions { display: flex; justify-content: flex-end; gap: 6px; }
  .actions button { padding: 4px 10px; border: 1px solid var(--panel-border); border-radius: 5px; background: var(--color-base-200); color: inherit; cursor: pointer; }
  .actions .primary { border-color: var(--color-primary); color: var(--color-primary); }
  .error { color: var(--color-error); font-size: 11px; overflow-wrap: anywhere; }
</style>
