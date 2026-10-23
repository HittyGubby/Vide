<script lang="ts">
  import { X } from "lucide-svelte";
  import { invoke } from "$lib/transport";
  import type { Song, Tag } from "$lib/types";

  let { songs, onclose, onsaved }: {
    songs: Song[];
    onclose: () => void;
    onsaved: (songs: Song[]) => void;
  } = $props();

  function tagText(song: Song) {
    return song.tags.map((tag) => tag.namespace + ":" + tag.value).join(" ");
  }

  let values = $state(songs.map(tagText).join("\n"));
  let busy = $state(false);
  let error = $state<string | null>(null);

  function parseLine(line: string): Tag[] {
    const tags: Tag[] = [];
    for (const token of line.trim().split(/\s+/).filter(Boolean)) {
      const index = token.indexOf(":");
      if (index <= 0 || index === token.length - 1) continue;
      tags.push({ namespace: token.slice(0, index), value: token.slice(index + 1) });
    }
    return tags;
  }

  async function save() {
    busy = true;
    error = null;
    try {
      const lines = values.split("\n");
      const updated: Song[] = [];
      for (let i = 0; i < songs.length; i += 1) {
        const desired = parseLine(lines[i] ?? "");
        const desiredKeys = new Set(desired.map((tag) => tag.namespace + ":" + tag.value));
        for (const tag of songs[i].tags) {
          if (!desiredKeys.has(tag.namespace + ":" + tag.value)) {
            await invoke("remove_tag", {
              songId: songs[i].id,
              namespace: tag.namespace,
              value: tag.value,
            });
          }
        }
        const existing = new Set(songs[i].tags.map((tag) => tag.namespace + ":" + tag.value));
        for (const tag of desired) {
          if (!existing.has(tag.namespace + ":" + tag.value)) {
            await invoke("add_tag", {
              songId: songs[i].id,
              namespace: tag.namespace,
              value: tag.value,
            });
          }
        }
        updated.push({ ...songs[i], tags: desired });
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
  <div class="modal" role="dialog" aria-label="批量编辑标签" onclick={(e) => e.stopPropagation()}>
    <div class="head">
      <h2>批量编辑标签 · {songs.length} 首</h2>
      <button class="icon" onclick={onclose} disabled={busy}><X size={16} /></button>
    </div>
    <p class="hint">每行对应一首歌，格式为 namespace:value，多个标签用空格分隔。留空会清除该歌曲标签。</p>
    <textarea bind:value={values} rows={Math.max(3, songs.length)} spellcheck="false"></textarea>
    {#if error}<p class="error">{error}</p>{/if}
    <div class="actions">
      <button onclick={onclose} disabled={busy}>取消</button>
      <button class="primary" onclick={() => void save()} disabled={busy}>{busy ? "保存中…" : "保存"}</button>
    </div>
  </div>
</div>

<style>
  .backdrop { position: fixed; inset: 0; z-index: 950; background: rgba(0,0,0,.55); display: flex; align-items: center; justify-content: center; }
  .modal { width: min(620px, calc(100vw - 32px)); max-height: calc(100vh - 48px); overflow: auto; display: flex; flex-direction: column; gap: 8px; padding: 16px; border-radius: 10px; background: var(--color-base-200); border: 1px solid var(--panel-border); }
  .head { display: flex; align-items: center; justify-content: space-between; }
  h2 { margin: 0; font-size: 15px; }
  .hint { margin: 0; font-size: 11px; color: var(--color-base-content) / 0.55; }
  textarea { resize: vertical; min-height: 90px; padding: 6px 8px; border: 1px solid var(--panel-border); border-radius: 5px; background: var(--color-base-100); color: inherit; font: inherit; font-size: 12px; }
  textarea:focus { outline: none; border-color: var(--color-primary); }
  .icon { border: 0; background: none; color: inherit; cursor: pointer; }
  .actions { display: flex; justify-content: flex-end; gap: 6px; }
  .actions button { padding: 4px 10px; border: 1px solid var(--panel-border); border-radius: 5px; background: var(--color-base-200); color: inherit; cursor: pointer; }
  .actions .primary { border-color: var(--color-primary); color: var(--color-primary); }
  .error { color: var(--color-error); font-size: 11px; overflow-wrap: anywhere; }
</style>
