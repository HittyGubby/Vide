<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { X, Plus } from "lucide-svelte";
  import type { Song, Tag } from "$lib/types";

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
  let input = $state("");
  let suggestions = $state<string[]>([]);
  let selectedSuggestion = $state(-1);
  let loading = $state(true);

  onMount(async () => {
    await loadTags();
    try {
      namespaces = await invoke("get_namespaces");
      metaTags = await invoke("get_meta_tags");
    } catch {
      /* ignore */
    }
    loading = false;
  });

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

    // Parse "namespace:value" or just "value"
    let ns: string;
    let val: string;
    const colon = trimmed.indexOf(":");
    if (colon > 0) {
      ns = trimmed.slice(0, colon).trim();
      val = trimmed.slice(colon + 1).trim();
    } else {
      // Default namespace "tag"
      ns = "tag";
      val = trimmed;
    }
    if (!ns || !val) return;

    // Skip if already present
    if (tags.some((t) => t.namespace === ns && t.value === val)) {
      input = "";
      suggestions = [];
      return;
    }

    try {
      await invoke("add_tag", { songId: song.id, namespace: ns, value: val });
      await loadTags();
      input = "";
      suggestions = [];

      // Add to local namespaces cache
      if (!namespaces.includes(ns)) {
        namespaces = [...namespaces, ns];
      }

      // Update the song's tags array in-place so the parent list re-renders
      song.tags = tags;
    } catch (e) {
      console.error("add tag error:", e);
    }
  }

  async function removeTag(ns: string, val: string) {
    try {
      await invoke("remove_tag", { songId: song.id, namespace: ns, value: val });
      await loadTags();
      song.tags = tags;
    } catch (e) {
      console.error("remove tag error:", e);
    }
  }

  // ── Autocomplete ──────────────────────────────────

  function updateSuggestions(value: string) {
    if (!value.trim()) {
      suggestions = [];
      selectedSuggestion = -1;
      return;
    }

    const colon = value.indexOf(":");
    if (colon < 0) {
      // Suggest namespaces
      const prefix = value.toLowerCase();
      suggestions = namespaces
        .filter((n) => n.toLowerCase().startsWith(prefix))
        .slice(0, 8);
    } else {
      // Suggest values for the given namespace
      const nsPart = value.slice(0, colon).trim().toLowerCase();
      const valPart = value.slice(colon + 1).toLowerCase();
      const allValues: string[] = [];

      // Collect values from metaTags
      for (const mt of metaTags) {
        if (mt.namespace.toLowerCase() === nsPart) {
          allValues.push(...mt.children);
        }
      }

      // Also collect from already loaded tags in this namespace
      for (const t of tags) {
        if (t.namespace.toLowerCase() === nsPart && !allValues.includes(t.value)) {
          allValues.push(t.value);
        }
      }

      suggestions = allValues
        .filter((v) => v.toLowerCase().startsWith(valPart))
        .slice(0, 8)
        .map((v) => `${value.slice(0, colon + 1)}${v}`);
    }
    selectedSuggestion = -1;
  }

  function selectSuggestion(idx: number) {
    const val = suggestions[idx];
    if (val) {
      input = val;
      suggestions = [];
    }
  }

  function handleInput(e: Event) {
    const target = e.target as HTMLInputElement;
    input = target.value;
    updateSuggestions(input);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      if (selectedSuggestion >= 0 && suggestions[selectedSuggestion]) {
        input = suggestions[selectedSuggestion];
        suggestions = [];
        addTag(input);
      } else {
        addTag(input);
      }
      e.preventDefault();
    } else if (e.key === "Escape") {
      if (suggestions.length > 0) {
        suggestions = [];
      } else {
        onclose();
      }
      e.preventDefault();
    } else if (e.key === "ArrowDown") {
      selectedSuggestion = Math.min(selectedSuggestion + 1, suggestions.length - 1);
      e.preventDefault();
    } else if (e.key === "ArrowUp") {
      selectedSuggestion = Math.max(selectedSuggestion - 1, 0);
      e.preventDefault();
    }
  }

  // ── Group tags by namespace ───────────────────────

  let grouped = $derived.by(() => {
    const map = new Map<string, Tag[]>();
    for (const t of tags) {
      const arr = map.get(t.namespace) ?? [];
      arr.push(t);
      map.set(t.namespace, arr);
    }
    return Array.from(map.entries()).sort(([a], [b]) => a.localeCompare(b));
  });
</script>

<!-- Backdrop (click to close) -->
<div class="backdrop" onclick={onclose} onkeydown={() => {}} role="presentation"></div>

<div class="tag-editor" role="dialog" aria-label="编辑标签">
  <div class="editor-header">
    <span class="editor-title">编辑标签</span>
    <span class="editor-song truncate">{song.title}</span>
  </div>

  {#if loading}
    <div class="loading">加载中...</div>
  {:else}
    <!-- Current tags -->
    <div class="tags-container">
      {#if tags.length === 0}
        <div class="no-tags">暂无标签</div>
      {:else}
        {#each grouped as [namespace, nsTags]}
          <div class="ns-group">
            <span class="ns-label">{namespace}</span>
            <div class="ns-values">
              {#each nsTags as tag}
                <span class="tag-badge">
                  <span class="tag-value">{tag.value}</span>
                  <button
                    class="tag-remove"
                    onclick={() => removeTag(tag.namespace, tag.value)}
                    aria-label="移除 {tag.namespace}:{tag.value}"
                  >
                    <X size={12} />
                  </button>
                </span>
              {/each}
            </div>
          </div>
        {/each}
      {/if}
    </div>

    <!-- Add tag input -->
    <div class="add-section">
      <div class="input-wrapper">
        <Plus size={14} class="add-icon" />
        <input
          type="text"
          class="tag-input"
          placeholder="namespace:value"
          value={input}
          oninput={handleInput}
          onkeydown={handleKeydown}
          onblur={() => setTimeout(() => (suggestions = []), 200)}
          autofocus
        />
      </div>

      {#if suggestions.length > 0}
        <div class="suggestions" role="listbox">
          {#each suggestions as suggestion, i}
            <button
              class="suggestion-item"
              class:selected={i === selectedSuggestion}
              onmousedown={() => selectSuggestion(i)}
              role="option"
              aria-selected={i === selectedSuggestion}
            >
              {suggestion}
            </button>
          {/each}
        </div>
      {/if}

      <button class="add-btn" onclick={() => addTag(input)} disabled={!input.trim()}>
        添加
      </button>
    </div>
  {/if}

  <div class="editor-footer">
    <span class="hint">Enter 添加 &middot; Esc 关闭</span>
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
    width: 320px;
    max-height: 380px;
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

  /* ── Tags Display ──────────────────────────────── */
  .tags-container {
    flex: 1;
    overflow-y: auto;
    padding: 10px 14px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .no-tags {
    font-size: 12px;
    color: var(--text-tertiary);
    text-align: center;
    padding: 8px 0;
  }

  .ns-group {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .ns-label {
    font-size: 10px;
    font-weight: 600;
    color: var(--text-tertiary);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .ns-values {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .tag-badge {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    padding: 2px 6px;
    border-radius: 4px;
    background: var(--accent-surface);
    color: var(--accent-primary);
    font-size: 11px;
  }

  .tag-remove {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    height: 14px;
    border-radius: 2px;
    color: var(--accent-primary);
    background: none;
    border: none;
    opacity: 0.6;
    transition: opacity 0.1s;
  }

  .tag-remove:hover {
    opacity: 1;
    background: rgba(255, 255, 255, 0.1);
  }

  /* ── Add Input ──────────────────────────────────── */
  .add-section {
    padding: 8px 14px 10px;
    border-top: 1px solid var(--border-subtle);
    display: flex;
    flex-direction: column;
    gap: 4px;
    position: relative;
  }

  .input-wrapper {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 8px;
    background: var(--bg-input);
    border: 1px solid var(--border-subtle);
    border-radius: 6px;
    transition: border-color 0.15s;
  }

  .input-wrapper:focus-within {
    border-color: var(--accent-primary);
  }

  .add-icon {
    color: var(--text-tertiary);
    flex-shrink: 0;
  }

  .tag-input {
    flex: 1;
    height: 20px;
    background: none;
    border: none;
    color: var(--text-primary);
    font-size: 12px;
    outline: none;
    user-select: text;
  }

  .tag-input::placeholder {
    color: var(--text-tertiary);
  }

  .add-btn {
    align-self: flex-end;
    padding: 4px 14px;
    border-radius: 5px;
    background: var(--accent-primary);
    color: white;
    font-size: 11px;
    font-weight: 500;
    border: none;
    transition: background 0.15s;
    margin-top: 4px;
  }

  .add-btn:hover:not(:disabled) {
    background: var(--accent-hover);
  }

  .add-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }

  /* ── Suggestions ────────────────────────────────── */
  .suggestions {
    position: absolute;
    bottom: 100%;
    left: 14px;
    right: 14px;
    background: var(--bg-elevated);
    border: 1px solid var(--border-normal);
    border-radius: 6px;
    box-shadow: 0 -4px 16px rgba(0, 0, 0, 0.3);
    max-height: 160px;
    overflow-y: auto;
    margin-bottom: 4px;
  }

  .suggestion-item {
    display: block;
    width: 100%;
    padding: 6px 10px;
    text-align: left;
    background: none;
    border: none;
    color: var(--text-primary);
    font-size: 12px;
    cursor: pointer;
  }

  .suggestion-item:hover,
  .suggestion-item.selected {
    background: var(--accent-surface);
    color: var(--accent-primary);
  }

  /* ── Footer ──────────────────────────────────────── */
  .editor-footer {
    padding: 6px 14px;
    border-top: 1px solid var(--border-subtle);
  }

  .hint {
    font-size: 10px;
    color: var(--text-tertiary);
  }
</style>
