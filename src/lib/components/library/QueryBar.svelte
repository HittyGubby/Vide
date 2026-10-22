<script lang="ts">
  import { onMount } from "svelte";
  import { Bookmark, Clock, Search, Star, X } from "lucide-svelte";
  import {
    libraryQuery,
    insertQueryFragment,
    splitSortClause,
  } from "$lib/stores/libraryQuery.svelte";
  import { QUERY_FIELDS } from "$lib/queryFields";
  import QueryInput from "$lib/components/library/QueryInput.svelte";

  // The editor edits a draft; Enter applies it to the list.
  let draft = $state("");
  let editorComp: {
    focus(): void;
    setValue(value: string, cursor?: number): void;
  } | null = null;
  let showDrop = $state(false);
  let dropTimer: ReturnType<typeof setTimeout> | undefined;
  let inputFocused = false;
  let panelHovered = false;

  let savedMatch = $derived(
    libraryQuery.saved.find((preset) => preset.query.trim() === draft.trim()),
  );
  let hasQuery = $derived(draft.trim().length > 0);
  let hasSaved = $derived(libraryQuery.saved.length > 0);
  let hasHistory = $derived(libraryQuery.history.length > 0);

  // Keep the bar in sync with the executed query (startup restore, locate
  // clearing the query, or another component applying a saved query).
  $effect(() => {
    draft = libraryQuery.query;
  });

  onMount(() => {
    void libraryQuery.load();

    const onKeyDown = (e: KeyboardEvent) => {
      if (e.repeat) return;
      const target = e.target as HTMLElement | null;
      const inField =
        target &&
        (target.tagName === "INPUT" ||
          target.tagName === "TEXTAREA" ||
          target.tagName === "SELECT" ||
          target.isContentEditable);

      // Ctrl/Cmd+F: turn the current query into
      //   (existing) AND song_metadata CONTAINS ""
      // with the caret inside the quotes, ready for typing. When the query
      // editor itself has focus, its own CodeMirror keymap handles Mod-f.
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "f") {
        if (inField) return;
        e.preventDefault();
        void insertRefine();
        return;
      }

      // "/" focuses the query bar unless the user is already typing.
      if (e.key === "/" && !inField) {
        e.preventDefault();
        focusBar();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  });

  function focusBar() {
    inputFocused = true;
    cancelClose();
    showDrop = true;
    editorComp?.focus();
  }

  function cancelClose() {
    if (dropTimer) {
      clearTimeout(dropTimer);
      dropTimer = undefined;
    }
  }

  function hidePanel() {
    cancelClose();
    showDrop = false;
    panelHovered = false;
  }

  /**
   * Close only when BOTH conditions hold: the editor lost focus and the
   * pointer is not over the panel. Clicking empty panel space blurs the
   * editor but keeps the panel open until the pointer leaves.
   */
  function maybeClosePanel() {
    if (inputFocused || panelHovered) return;
    cancelClose();
    dropTimer = setTimeout(() => {
      if (!inputFocused && !panelHovered) showDrop = false;
    }, 120);
  }

  function onEditorFocus() {
    inputFocused = true;
    cancelClose();
    showDrop = true;
  }

  function onEditorBlur() {
    inputFocused = false;
    maybeClosePanel();
  }

  function onPanelEnter() {
    panelHovered = true;
    cancelClose();
  }

  function onPanelLeave() {
    panelHovered = false;
    maybeClosePanel();
  }

  async function execute(query: string = draft) {
    const next = query.trim();
    draft = next;
    hidePanel();
    await libraryQuery.execute(next);
  }

  async function apply(query: string) {
    await execute(query);
  }

  async function clear() {
    draft = "";
    await execute("");
  }

  async function toggleSavedFor(query: string) {
    const trimmed = query.trim();
    if (!trimmed) return;
    const existing = libraryQuery.savedFor(trimmed);
    if (existing) {
      await libraryQuery.remove(existing.id);
      return;
    }
    // One click saves it; the query text doubles as the display name and is
    // ellipsized in the dropdown. A rename UI can come later.
    await libraryQuery.save(trimmed, trimmed);
  }

  async function toggleSaved() {
    const query = draft.trim();
    if (!query) return;
    await toggleSavedFor(query);
  }

  function insertRefine() {
    // Insert before a trailing SORT BY clause so the directive stays last.
    const next = insertQueryFragment(
      draft,
      'song_metadata CONTAINS ""',
      " AND ",
    );
    draft = next;
    showDrop = false;
    const pos = next.lastIndexOf('""');
    editorComp?.setValue(next, pos >= 0 ? pos + 1 : undefined);
  }

  function onEditorChange(next: string) {
    draft = next;
    // Keep the panel available when the query is cleared; typing hides it so
    // the CodeMirror completion popup has the space to itself.
    if (next.trim().length === 0) {
      showDrop = true;
    } else {
      hidePanel();
    }
  }

  /** Insert one of the metadata keys when the empty bar is showing its help. */
  function insertField(field: string) {
    const next = insertQueryFragment(draft, field + " ");
    const { filter, sort } = splitSortClause(next);
    draft = next;
    showDrop = false;
    editorComp?.setValue(next, sort ? filter.length + 1 : next.length);
  }
</script>

<div class="query-bar" data-tauri-drag-region="false">
  <div class="query-field">
    <span class="q-icon"><Search size={14} /></span>
    <QueryInput
      bind:this={editorComp}
      value={draft}
      onchange={onEditorChange}
      onsubmit={() => void execute()}
      onrefine={insertRefine}
      onfocus={onEditorFocus}
      onblur={onEditorBlur}
    />
    {#if hasQuery}
      <button
        class="q-btn"
        class:saved={savedMatch}
        title={savedMatch ? "取消收藏" : "收藏当前查询"}
        aria-label={savedMatch ? "取消收藏" : "收藏当前查询"}
        onmousedown={(e) => e.preventDefault()}
        onclick={toggleSaved}
      >
        <Star size={14} fill={savedMatch ? "currentColor" : "none"} />
      </button>
      <button
        class="q-btn"
        title="清空查询"
        aria-label="清空查询"
        onmousedown={(e) => e.preventDefault()}
        onclick={clear}
      >
        <X size={13} />
      </button>
    {/if}
  </div>

  {#if showDrop}
    <div
      class="q-drop"
      role="listbox"
      tabindex="-1"
      onmouseenter={onPanelEnter}
      onmouseleave={onPanelLeave}
    >
      <!-- Left: saved queries. Right: history. Each history item can be
           starred into the left column; each saved item can be deleted. -->
      <div class="drop-column">
        <div class="drop-title">
          <Bookmark size={12} />
          <span>已保存</span>
        </div>
        {#if hasSaved}
          {#each libraryQuery.saved as preset}
            <div class="drop-row">
              <button
                class="drop-main"
                title={preset.query}
                onmousedown={(e) => e.preventDefault()}
                onclick={() => apply(preset.query)}
              >
                <span class="row-name">{preset.name}</span>
                {#if preset.name !== preset.query}
                  <span class="row-query">{preset.query}</span>
                {/if}
              </button>
              <button
                class="row-del"
                title="删除收藏"
                aria-label="删除收藏"
                onmousedown={(e) => e.preventDefault()}
                onclick={() => libraryQuery.remove(preset.id)}
              >
                <X size={12} />
              </button>
            </div>
          {/each}
        {:else}
          <div class="drop-empty">暂无收藏</div>
        {/if}
      </div>

      <div class="drop-column">
        <div class="drop-title">
          <Clock size={12} />
          <span>历史</span>
          {#if hasHistory}
            <button
              class="drop-clear"
              onmousedown={(e) => e.preventDefault()}
              onclick={() => libraryQuery.clearHistory()}
            >
              清除
            </button>
          {/if}
        </div>
        {#if hasHistory}
          {#each libraryQuery.history as query}
            {@const isSaved = libraryQuery.savedFor(query)}
            <div class="drop-row">
              <button
                class="drop-main history"
                title={query}
                onmousedown={(e) => e.preventDefault()}
                onclick={() => apply(query)}
              >
                {query}
              </button>
              <button
                class="row-fav"
                class:saved={isSaved}
                title={isSaved ? "取消收藏" : "收藏此查询"}
                aria-label={isSaved ? "取消收藏" : "收藏此查询"}
                onmousedown={(e) => e.preventDefault()}
                onclick={() => toggleSavedFor(query)}
              >
                <Star size={12} fill={isSaved ? "currentColor" : "none"} />
              </button>
              <button
                class="row-del"
                title="删除这条历史"
                aria-label="删除这条历史"
                onmousedown={(e) => e.preventDefault()}
                onclick={() => libraryQuery.removeHistory(query)}
              >
                <X size={12} />
              </button>
            </div>
          {/each}
        {:else}
          <div class="drop-empty">暂无历史</div>
        {/if}
      </div>

      <!-- With nothing typed, show every queryable key so the user does not
           have to remember them. -->
      <div class="drop-fields">
        <span class="drop-fields-label">字段</span>
        {#each QUERY_FIELDS as field}
          <button
            class="field-chip"
            title="插入 {field}"
            onmousedown={(e) => e.preventDefault()}
            onclick={() => insertField(field)}
          >
            {field}
          </button>
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  .query-bar {
    position: relative;
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
  }

  .query-field {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    height: 30px;
    padding: 0 8px;
    background: var(--color-base-100);
    border: 1px solid var(--panel-border);
    border-radius: 6px;
    transition: border-color 0.15s;
  }
  .query-field:focus-within {
    border-color: var(--color-primary);
  }

  .q-icon {
    display: flex;
    flex: none;
    color: var(--color-base-content) / 0.5;
  }


  .q-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    flex: none;
    width: 22px;
    height: 22px;
    border-radius: 4px;
    background: none;
    border: none;
    color: var(--color-base-content) / 0.55;
  }
  .q-btn:hover {
    background: var(--color-base-300);
    color: var(--color-base-content);
  }
  .q-btn.saved {
    color: var(--color-primary);
  }

  .q-drop {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    right: 0;
    z-index: 300;
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    padding: 4px;
    background: var(--color-base-300);
    border: 1px solid var(--panel-border-strong);
    border-radius: 8px;
    box-shadow: 0 8px 26px rgba(0, 0, 0, 0.45);
  }

  /* Two columns: saved queries on the left, history on the right. */
  .drop-column {
    flex: 1 1 0;
    min-width: 0;
    max-height: 340px;
    overflow-y: auto;
  }
  .drop-column + .drop-column {
    border-left: 1px solid var(--panel-border);
    padding-left: 6px;
  }
  .drop-fields {
    flex: 0 0 100%;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
    margin-top: 2px;
    padding: 6px 6px 4px;
    border-top: 1px solid var(--panel-border);
  }
  .drop-fields-label {
    margin-right: 2px;
    font-size: 10px;
    color: var(--color-base-content) / 0.45;
  }
  .field-chip {
    padding: 1px 6px;
    border-radius: 4px;
    background: var(--color-base-100);
    border: 1px solid var(--panel-border);
    color: var(--color-base-content) / 0.72;
    font-size: 10.5px;
    line-height: 1.5;
  }
  .field-chip:hover {
    border-color: var(--color-primary);
    color: var(--color-primary);
  }

  .drop-title {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 8px;
    font-size: 11px;
    color: var(--color-base-content) / 0.55;
  }

  .drop-clear {
    margin-left: auto;
    padding: 1px 4px;
    background: none;
    border: none;
    color: var(--color-base-content) / 0.55;
    font-size: 11px;
  }
  .drop-clear:hover {
    color: var(--color-base-content);
  }

  .drop-row {
    display: flex;
    align-items: center;
    min-width: 0;
  }

  .drop-main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding: 5px 8px;
    background: none;
    border: none;
    border-radius: 4px;
    color: var(--color-base-content);
    text-align: left;
    font-size: 12.5px;
  }
  .drop-main:hover {
    background: var(--color-primary) / 0.12;
  }
  .drop-main.history {
    display: block;
    width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12px;
  }

  .row-name,
  .row-query {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .row-name {
    font-size: 12.5px;
  }
  .row-query {
    font-size: 10.5px;
    color: var(--color-base-content) / 0.55;
  }

  .row-del,
  .row-fav {
    display: flex;
    align-items: center;
    justify-content: center;
    flex: none;
    width: 24px;
    height: 24px;
    margin-right: 2px;
    border-radius: 4px;
    background: none;
    border: none;
    color: var(--color-base-content) / 0.4;
  }
  .row-del:hover {
    color: var(--color-error);
    background: var(--color-base-100);
  }
  .row-fav:hover {
    color: var(--color-primary);
    background: var(--color-base-100);
  }
  .row-fav.saved {
    color: var(--color-primary);
  }

  .drop-empty {
    padding: 8px;
    font-size: 11px;
    color: var(--color-base-content) / 0.55;
    text-align: center;
  }
</style>
