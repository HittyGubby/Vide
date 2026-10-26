<script lang="ts">
  import { Plus } from "lucide-svelte";
  import type { Tag } from "$lib/types";

  interface TagGroup {
    namespace: string;
    children: string[];
  }

  /**
   * Tag input with namespace/value completion. The suggestion logic needs the
   * known namespaces, the catalog and the tags already on this song.
   */
  let {
    namespaces,
    metaTags,
    existingTags,
    onadd,
    onclose,
  }: {
    namespaces: string[];
    metaTags: TagGroup[];
    existingTags: Tag[];
    onadd: (raw: string) => void;
    onclose: () => void;
  } = $props();

  let input = $state("");
  let suggestions = $state<string[]>([]);
  let selectedSuggestion = $state(-1);

  function updateSuggestions(value: string) {
    if (!value.trim()) {
      suggestions = [];
      selectedSuggestion = -1;
      return;
    }

    const colon = value.indexOf(":");
    if (colon < 0) {
      const prefix = value.toLowerCase();
      suggestions = namespaces
        .filter((namespace) => namespace.toLowerCase().startsWith(prefix))
        .slice(0, 8);
    } else {
      const nsPart = value.slice(0, colon).trim().toLowerCase();
      const valPart = value.slice(colon + 1).toLowerCase();
      const allValues: string[] = [];
      for (const group of metaTags) {
        if (group.namespace.toLowerCase() === nsPart) allValues.push(...group.children);
      }
      for (const tag of existingTags) {
        if (tag.namespace.toLowerCase() === nsPart && !allValues.includes(tag.value)) {
          allValues.push(tag.value);
        }
      }
      suggestions = allValues
        .filter((item) => item.toLowerCase().startsWith(valPart))
        .slice(0, 8)
        .map((item) => value.slice(0, colon + 1) + item);
    }
    selectedSuggestion = -1;
  }

  function onInput(e: Event) {
    input = (e.target as HTMLInputElement).value;
    updateSuggestions(input);
  }

  function selectSuggestion(index: number) {
    const value = suggestions[index];
    if (!value) return;
    input = value;
    suggestions = [];
  }

  function submit() {
    const value = input.trim();
    if (!value) return;
    onadd(value);
    input = "";
    suggestions = [];
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Tab") {
      if (suggestions.length > 0) {
        const chosen = suggestions[selectedSuggestion >= 0 ? selectedSuggestion : 0];
        input = chosen.includes(":") ? chosen : chosen + ":";
        suggestions = [];
        selectedSuggestion = -1;
      }
      e.preventDefault();
    } else if (e.key === "Enter") {
      if (selectedSuggestion >= 0 && suggestions[selectedSuggestion]) {
        input = suggestions[selectedSuggestion];
        suggestions = [];
      }
      submit();
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
</script>

<div class="add-section">
  <div class="input-wrapper">
    <Plus size={14} class="add-icon" />
    <input
      type="text"
      class="tag-input"
      placeholder="namespace:value"
      value={input}
      oninput={onInput}
      onkeydown={onKeydown}
      onblur={() => setTimeout(() => (suggestions = []), 200)}
      autofocus
    />
  </div>

  {#if suggestions.length > 0}
    <div class="suggestions" role="listbox">
      {#each suggestions as suggestion, index}
        <button
          class="suggestion-item"
          class:selected={index === selectedSuggestion}
          onmousedown={() => selectSuggestion(index)}
          role="option"
          aria-selected={index === selectedSuggestion}
        >
          {suggestion}
        </button>
      {/each}
    </div>
  {/if}

  <button class="add-btn" onclick={submit} disabled={!input.trim()}>
    添加
  </button>
</div>

<style>
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
</style>
