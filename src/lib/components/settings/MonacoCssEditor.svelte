<script lang="ts">
  import { onMount, onDestroy } from "svelte";

  interface Props {
    value: string;
    /** Called (debounced) whenever the document changes. */
    onchange: (next: string) => void;
    heightPx?: number;
  }

  let { value, onchange, heightPx = 320 }: Props = $props();

  type Editor = import("monaco-editor").editor.IStandaloneCodeEditor;

  let hostEl: HTMLDivElement;
  let editor: Editor | null = null;
  let applyingExternal = false;
  let changeTimer: ReturnType<typeof setTimeout> | null = null;
  let ready = $state(false);
  let failure = $state<string | null>(null);

  onMount(async () => {
    // Monaco touches `document` on import, so it is only loaded in the browser.
    try {
      const [monaco, editorWorkerMod, cssWorkerMod] = await Promise.all([
        import("monaco-editor"),
        // These specifiers go through the package `exports` map
        // ("./*" -> "./esm/vs/*.js"), which is the only form Rollup resolves.
        import("monaco-editor/editor/editor.worker?worker"),
        import("monaco-editor/language/css/css.worker?worker"),
      ]);
      const EditorWorker = editorWorkerMod.default;
      const CssWorker = cssWorkerMod.default;
      (self as unknown as { MonacoEnvironment: unknown }).MonacoEnvironment = {
        getWorker(_id: string, label: string) {
          if (label === "css" || label === "scss" || label === "less") return new CssWorker();
          return new EditorWorker();
        },
      };

      editor = monaco.editor.create(hostEl, {
        value,
        language: "css",
        theme: "vs-dark",
        automaticLayout: true,
        minimap: { enabled: false },
        fontSize: 12,
        lineNumbers: "on",
        scrollBeyondLastLine: false,
        tabSize: 2,
        wordWrap: "on",
      });

      editor.onDidChangeModelContent(() => {
        if (applyingExternal || !editor) return;
        const next = editor.getValue();
        if (changeTimer) clearTimeout(changeTimer);
        changeTimer = setTimeout(() => onchange(next), 300);
      });

      ready = true;
    } catch (e) {
      console.error("monaco load error:", e);
      failure = String(e);
    }
  });

  // Adopt external edits (e.g. settings loaded from the backend) without
  // echoing them back as user changes.
  $effect(() => {
    const next = value;
    if (!editor || !ready) return;
    if (editor.getValue() === next) return;
    applyingExternal = true;
    editor.setValue(next);
    applyingExternal = false;
  });

  onDestroy(() => {
    if (changeTimer) clearTimeout(changeTimer);
    editor?.dispose();
    editor = null;
  });
</script>

<div class="monaco-host" bind:this={hostEl} style="height: {heightPx}px"></div>
{#if !ready && !failure}
  <p class="monaco-note">编辑器加载中…</p>
{:else if failure}
  <p class="monaco-note error">编辑器加载失败：{failure}</p>
{/if}

<style>
  .monaco-host {
    width: 100%;
    border: 1px solid var(--panel-border);
    border-radius: 8px;
    overflow: hidden;
  }
  .monaco-note {
    font-size: 12px;
    color: color-mix(in oklch, var(--color-base-content) 55%, transparent);
    padding: 6px 2px;
  }
  .monaco-note.error { color: var(--color-error, #f87171); }
</style>