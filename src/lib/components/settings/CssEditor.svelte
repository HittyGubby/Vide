<script lang="ts">
  import { onMount, onDestroy } from "svelte";

  interface Props {
    value: string;
    /** Called (debounced) whenever the document changes. */
    onchange: (next: string) => void;
    heightPx?: number;
  }

  let { value, onchange, heightPx = 320 }: Props = $props();

  // Type-only: CodeMirror is loaded in onMount so prerendering never touches
  // the DOM and the editor stays in its own chunk.
  type EditorView = import("@codemirror/view").EditorView;

  let hostEl: HTMLDivElement;
  let view: EditorView | null = null;
  let applyingExternal = false;
  let changeTimer: ReturnType<typeof setTimeout> | null = null;
  let ready = $state(false);
  let failure = $state<string | null>(null);

  onMount(async () => {
    try {
      const [{ EditorView, basicSetup }, { css }, { oneDark }] = await Promise.all([
        import("codemirror"),
        import("@codemirror/lang-css"),
        import("@codemirror/theme-one-dark"),
      ]);

      view = new EditorView({
        parent: hostEl,
        doc: value,
        extensions: [
          basicSetup,
          css(),
          oneDark,
          EditorView.theme({
            "&": { height: "100%", fontSize: "12px" },
            ".cm-scroller": { overflow: "auto" },
          }),
          EditorView.updateListener.of((update) => {
            if (!update.docChanged || applyingExternal) return;
            const next = update.state.doc.toString();
            if (changeTimer) clearTimeout(changeTimer);
            changeTimer = setTimeout(() => onchange(next), 300);
          }),
        ],
      });

      ready = true;
    } catch (e) {
      console.error("codemirror load error:", e);
      failure = String(e);
    }
  });

  // Adopt external edits (e.g. settings loaded from the backend) without
  // echoing them back as user changes.
  $effect(() => {
    const next = value;
    if (!view || !ready) return;
    if (view.state.doc.toString() === next) return;
    applyingExternal = true;
    view.dispatch({
      changes: { from: 0, to: view.state.doc.length, insert: next },
    });
    applyingExternal = false;
  });

  onDestroy(() => {
    if (changeTimer) clearTimeout(changeTimer);
    view?.destroy();
    view = null;
  });
</script>

<div class="css-editor" bind:this={hostEl} style="height: {heightPx}px"></div>
{#if !ready && !failure}
  <p class="css-editor-note">编辑器加载中…</p>
{:else if failure}
  <p class="css-editor-note error">编辑器加载失败：{failure}</p>
{/if}

<style>
  .css-editor {
    width: 100%;
    border: 1px solid var(--panel-border);
    border-radius: 8px;
    overflow: hidden;
    background: #282c34;
  }
  .css-editor-note {
    font-size: 12px;
    color: color-mix(in oklch, var(--color-base-content) 55%, transparent);
    padding: 6px 2px;
  }
  .css-editor-note.error {
    color: var(--color-error, #f87171);
  }
</style>
