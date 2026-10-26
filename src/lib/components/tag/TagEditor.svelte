<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "$lib/transport";
  import { canManage } from "$lib/stores/auth.svelte";
  import type { Song, Tag } from "$lib/types";
  import TagSelected from "$lib/components/tag/TagSelected.svelte";
  import TagHistory from "$lib/components/tag/TagHistory.svelte";
  import TagCatalog from "$lib/components/tag/TagCatalog.svelte";
  import TagInput from "$lib/components/tag/TagInput.svelte";

  let {
    song,
    onclose,
  }: {
    song: Song;
    onclose: () => void;
  } = $props();

  let tags = $state<Tag[]>([]);
  let namespaces = $state<string[]>([]);
  let metaTags = $state<{ namespace: string; children: string[] }[]>([]);
  let loading = $state(true);
  let history = $state<Tag[]>([]);
  const HISTORY_KEY = "vide_tag_history";

  onMount(async () => {
    await loadTags();
    await loadCatalog();
    await loadHistory();
    loading = false;
  });

  async function loadCatalog() {
    try {
      namespaces = await invoke("get_namespaces");
      metaTags = await invoke("get_meta_tags");
    } catch {
      /* ignore */
    }
  }

  function localHistory(): Tag[] {
    try {
      const parsed = JSON.parse(localStorage.getItem(HISTORY_KEY) ?? "[]");
      return Array.isArray(parsed) ? parsed : [];
    } catch {
      return [];
    }
  }

  function saveLocalHistory(entries: Tag[]) {
    localStorage.setItem(HISTORY_KEY, JSON.stringify(entries.slice(0, 50)));
  }

  async function loadHistory() {
    // Guests (local Tauri is never a guest) keep their usage history in the
    // browser, because there is no session to write it to the server.
    if (!canManage()) {
      history = localHistory();
      return;
    }
    try {
      history = await invoke<Tag[]>("get_tag_history");
      saveLocalHistory(history);
    } catch {
      history = localHistory();
    }
  }

  function rememberLocal(namespace: string, value: string) {
    if (canManage()) return;
    const next = [
      { namespace, value },
      ...localHistory().filter((tag) => tag.namespace !== namespace || tag.value !== value),
    ];
    history = next.slice(0, 50);
    saveLocalHistory(history);
  }

  async function loadTags() {
    try {
      tags = await invoke("get_song_tags", { songId: song.id });
    } catch {
      tags = [];
    }
  }

  async function addTag(raw: string) {
    const trimmed = raw.trim();
    if (!trimmed) return;

    // Parse "namespace:value" or just "value" (default namespace "tag").
    let namespace: string;
    let value: string;
    const colon = trimmed.indexOf(":");
    if (colon > 0) {
      namespace = trimmed.slice(0, colon).trim();
      value = trimmed.slice(colon + 1).trim();
    } else {
      namespace = "tag";
      value = trimmed;
    }
    if (!namespace || !value) return;
    if (tags.some((tag) => tag.namespace === namespace && tag.value === value)) return;

    try {
      await invoke("add_tag", { songId: song.id, namespace, value });
      await loadTags();
      if (!namespaces.includes(namespace)) {
        namespaces = [...namespaces, namespace];
      }
      song.tags = tags;

      if (!canManage()) {
        rememberLocal(namespace, value);
      } else {
        await loadHistory();
        saveLocalHistory(history);
      }

      const existing = metaTags.find((entry) => entry.namespace === namespace);
      if (existing) {
        if (!existing.children.includes(value)) {
          metaTags = metaTags.map((entry) =>
            entry.namespace === namespace
              ? { ...entry, children: [...entry.children, value].sort() }
              : entry,
          );
        }
      } else {
        metaTags = [...metaTags, { namespace, children: [value] }];
      }
      namespaces = namespaces.includes(namespace) ? namespaces : [...namespaces, namespace];
    } catch (e) {
      console.error("add tag error:", e);
    }
  }

  async function removeTag(namespace: string, value: string) {
    try {
      await invoke("remove_tag", { songId: song.id, namespace, value });
      await loadTags();
      song.tags = tags;
    } catch (e) {
      console.error("remove tag error:", e);
    }
  }

  /** Forget one entry from the usage history; the song tags stay untouched. */
  async function removeHistory(tag: Tag) {
    history = history.filter(
      (entry) => entry.namespace !== tag.namespace || entry.value !== tag.value,
    );
    saveLocalHistory(history);
    if (!canManage()) return;
    try {
      await invoke("remove_tag_history", {
        namespace: tag.namespace,
        value: tag.value,
      });
    } catch (e) {
      console.error("remove tag history error:", e);
      await loadHistory();
    }
  }

  // Group the song's tags by namespace for display.
  let grouped = $derived.by(() => {
    const map = new Map<string, Tag[]>();
    for (const tag of tags) {
      const list = map.get(tag.namespace) ?? [];
      list.push(tag);
      map.set(tag.namespace, list);
    }
    return Array.from(map.entries()).sort(([a], [b]) => a.localeCompare(b));
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="backdrop" onclick={onclose} onkeydown={() => {}} role="presentation"></div>

<div class="tag-editor" role="dialog" aria-label="编辑标签">
  <div class="editor-header">
    <span class="editor-title">编辑标签</span>
    <span class="editor-song truncate">{song.title}</span>
  </div>

  {#if loading}
    <div class="loading">加载中...</div>
  {:else}
    <div class="tags-container">
      <TagSelected groups={grouped} onremove={(namespace, value) => void removeTag(namespace, value)} />
      <TagHistory
        {history}
        onadd={(tag) => void addTag(tag.namespace + ":" + tag.value)}
        onremove={(tag) => void removeHistory(tag)}
      />
      <TagCatalog groups={metaTags} onadd={(namespace, value) => void addTag(namespace + ":" + value)} />
    </div>

    <TagInput
      {namespaces}
      {metaTags}
      existingTags={tags}
      onadd={(raw) => void addTag(raw)}
      {onclose}
    />
  {/if}

  <div class="editor-footer">
    <span class="hint">Tab 补全 &middot; Enter 添加 &middot; Esc 关闭</span>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 500;
    background: transparent;
  }

  .tag-editor {
    position: fixed;
    z-index: 501;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: 460px;
    max-height: 640px;
    background: var(--bg-elevated);
    border: 1px solid var(--border-normal);
    border-radius: 10px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .editor-header {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 12px 14px 8px;
    border-bottom: 1px solid var(--border-subtle);
  }

  .editor-title {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary);
  }

  .editor-song {
    font-size: 11px;
    color: var(--text-tertiary);
  }

  .loading {
    padding: 20px;
    text-align: center;
    color: var(--text-tertiary);
    font-size: 12px;
  }

  .tags-container {
    flex: 1;
    overflow-y: auto;
    padding: 10px 14px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .editor-footer {
    padding: 6px 14px;
    border-top: 1px solid var(--border-subtle);
  }

  .hint {
    font-size: 10px;
    color: var(--text-tertiary);
  }
</style>
