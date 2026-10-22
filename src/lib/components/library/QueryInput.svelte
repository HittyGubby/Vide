<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { invoke } from "$lib/transport";
  import { EditorState } from "@codemirror/state";
  import { EditorView, drawSelection, keymap, placeholder } from "@codemirror/view";
  import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
  import {
    acceptCompletion,
    autocompletion,
    closeBrackets,
    closeBracketsKeymap,
    closeCompletion as closeCompletionCommand,
    moveCompletionSelection,
    startCompletion,
    type Completion,
    type CompletionContext,
    type CompletionResult,
  } from "@codemirror/autocomplete";
  import {
    HighlightStyle,
    StreamLanguage,
    syntaxHighlighting,
  } from "@codemirror/language";
  import { tags } from "@lezer/highlight";
  import {
    QUERY_FIELDS,
    QUERY_KEYWORDS,
    QUERY_OPERATORS,
  } from "$lib/queryFields";

  interface Props {
    value: string;
    onchange: (value: string) => void;
    onsubmit: () => void;
    onrefine: () => void;
    onfocus: () => void;
    onblur: () => void;
  }

  let { value, onchange, onsubmit, onrefine, onfocus, onblur }: Props = $props();

  let hostEl: HTMLDivElement;
  let view: EditorView | null = null;
  let applying = false;
  let failure = $state<string | null>(null);

  const FIELD_TOKEN = new RegExp(
    "^(" + QUERY_FIELDS.join("|") + ")(?![A-Za-z0-9_])",
    "i",
  );
  const KEYWORD_TOKEN = new RegExp(
    "^(" + QUERY_KEYWORDS.join("|") + ")(?![A-Za-z0-9_])",
    "i",
  );

  const queryLanguage = StreamLanguage.define({
    startState: () => ({}),
    token(stream) {
      if (stream.eatSpace()) return null;
      if (stream.match(/^"(?:\\.|[^"\\])*"?/)) return "string";
      if (stream.match(KEYWORD_TOKEN)) return "keyword";
      // A namespace before a colon is a tag key, e.g. genre:folk.
      if (stream.match(/^[A-Za-z0-9_-]+(?=:)/)) return "className";
      if (stream.match(FIELD_TOKEN)) return "propertyName";
      if (stream.match(/^(>=|<=|!=|=|>|<|:)/)) return "operator";
      if (stream.match(/^[()]/)) return "punctuation";
      if (stream.match(/^[^\s()]+/)) return "variableName";
      stream.next();
      return null;
    },
  });

  const highlightStyle = HighlightStyle.define([
    { tag: tags.keyword, color: "var(--color-primary)", fontWeight: "600" },
    { tag: tags.propertyName, color: "var(--color-info, #38bdf8)" },
    { tag: tags.className, color: "var(--color-secondary, #c084fc)" },
    { tag: tags.string, color: "var(--color-success, #4ade80)" },
    { tag: tags.operator, color: "var(--color-warning, #fbbf24)" },
    { tag: tags.punctuation, color: "var(--color-base-content)" },
    { tag: tags.variableName, color: "var(--color-base-content)" },
  ]);

  let namespacesCache: string[] | null = null;

  async function namespaces(): Promise<string[]> {
    if (namespacesCache) return namespacesCache;
    try {
      namespacesCache = await invoke<string[]>("get_namespaces");
    } catch {
      namespacesCache = [];
    }
    return namespacesCache;
  }

  /** Completion that inserts a value with quotes/backslashes escaped. */
  function valueCompletion(label: string): Completion {
    return {
      label,
      type: "text",
      apply: (editor, _completion, from, to) => {
        const escaped = label.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
        editor.dispatch({
          changes: { from, to, insert: escaped },
          selection: { anchor: from + escaped.length },
        });
      },
    };
  }

  async function fieldValues(field: string, prefix: string): Promise<string[]> {
    try {
      return await invoke<string[]>("query_field_values", {
        field,
        prefix,
        limit: 20,
      });
    } catch {
      return [];
    }
  }

  async function tagValues(namespace: string, prefix: string): Promise<string[]> {
    try {
      return await invoke<string[]>("tag_values", {
        namespace,
        prefix,
        limit: 20,
      });
    } catch {
      return [];
    }
  }

  async function completionSource(
    context: CompletionContext,
  ): Promise<CompletionResult | null> {
    const before = context.state.sliceDoc(0, context.pos);

    // Inside a CONTAINS "..." value: suggest values of that field.
    const contains = /([A-Za-z_][A-Za-z0-9_]*)\s+CONTAINS\s+"([^"]*)$/i.exec(before);
    if (contains) {
      const field = contains[1].toLowerCase();
      const prefix = contains[2];
      const values = await fieldValues(field, prefix);
      if (!values.length) return null;
      return {
        from: context.pos - prefix.length,
        options: values.map(valueCompletion),
        validFor: /^[\w .:/-]*$/,
      };
    }

    // After "namespace:": suggest values of that tag namespace.
    const tag = /(?:^|[\s(])([A-Za-z0-9_-]+):([A-Za-z0-9_.-]*)$/.exec(before);
    if (tag) {
      const prefix = tag[2];
      const values = await tagValues(tag[1], prefix);
      if (!values.length) return null;
      return {
        from: context.pos - prefix.length,
        options: values.map(valueCompletion),
        validFor: /^[\w.-]*$/,
      };
    }

    // SORT BY <field> [ASC|DESC] completion (the directive is parsed by the
    // library query store, not by the filter grammar).
    const sortDirection = /(?:^|\s)SORT\s+BY\s+[A-Za-z_][A-Za-z0-9_]*\s+([A-Za-z]*)$/i.exec(
      before,
    );
    if (sortDirection) {
      const prefix = sortDirection[1].toLowerCase();
      const options = ["ASC", "DESC"]
        .filter((option) => option.toLowerCase().startsWith(prefix))
        .map((label) => ({ label, type: "keyword" }));
      return options.length
        ? {
            from: context.pos - sortDirection[1].length,
            options,
            validFor: /^[A-Za-z]*$/,
          }
        : null;
    }

    const sortFieldPrefix = /(?:^|\s)SORT\s+BY\s+([A-Za-z0-9_]*)$/i.exec(before);
    if (sortFieldPrefix) {
      const prefix = sortFieldPrefix[1].toLowerCase();
      const options = QUERY_FIELDS.filter((field) =>
        field.startsWith(prefix),
      ).map((field) => ({ label: field, type: "property" }));
      return options.length
        ? {
            from: context.pos - sortFieldPrefix[1].length,
            options,
            validFor: /^[\w]*$/,
          }
        : null;
    }

    if (/(?:^|\s)SORT\s+$/i.test(before)) {
      return {
        from: context.pos,
        options: [{ label: "BY", type: "keyword" }],
        validFor: /^[A-Za-z]*$/,
      };
    }

    const word = context.matchBefore(/[\w:-]*$/) ?? {
      from: context.pos,
      to: context.pos,
      text: "",
    };

    // After a field name, the only useful next word is CONTAINS. Check this
    // before the empty-word guard so the suggestion still appears after a
    // space.
    const afterField = /(?:^|[\s(])([A-Za-z_][A-Za-z0-9_]*)\s+$/.exec(before);
    if (afterField && QUERY_FIELDS.includes(afterField[1].toLowerCase())) {
      return {
        from: word?.from ?? context.pos,
        options: [
          { label: "CONTAINS", type: "keyword" },
          ...QUERY_OPERATORS.map((operator) => ({
            label: operator,
            type: "operator",
          })),
        ],
        validFor: /^[\w]*$/,
      };
    }

    const ns = await namespaces();
    const lower = word.text.toLowerCase();
    const options: Completion[] = [
      ...QUERY_FIELDS.map((label) => ({ label, type: "property" })),
      ...QUERY_KEYWORDS.map((label) => ({ label, type: "keyword" })),
      ...ns.map((namespace) => ({ label: namespace + ":", type: "namespace" })),
    ].filter((option) => option.label.toLowerCase().startsWith(lower));
    if (!options.length) return null;
    return { from: word.from, options, validFor: /^[\w:-]*$/ };
  }

  const theme = EditorView.theme({
    "&": {
      height: "30px",
      fontSize: "12.5px",
      color: "var(--color-base-content)",
      backgroundColor: "transparent",
    },
    ".cm-scroller": {
      fontFamily: "inherit",
      lineHeight: "28px",
      overflowX: "auto",
      overflowY: "hidden",
    },
    ".cm-content": {
      padding: "0",
      caretColor: "var(--color-primary)",
      minHeight: "28px",
    },
    ".cm-line": { padding: "0" },
    "&.cm-focused": { outline: "none" },
    ".cm-tooltip": {
      // Must sit above the favorites/history panel (z-index: 300).
      zIndex: "400",
      backgroundColor: "var(--color-base-300)",
      border: "1px solid var(--panel-border-strong)",
      borderRadius: "6px",
      color: "var(--color-base-content)",
    },
    ".cm-tooltip-autocomplete ul li[aria-selected]": {
      backgroundColor: "var(--color-primary)",
      color: "var(--color-primary-content)",
    },
    ".cm-placeholder": {
      color: "var(--color-base-content)",
      opacity: "0.42",
    },
  });

  onMount(() => {
    try {
      const state = EditorState.create({
        doc: value,
        extensions: [
          history(),
          drawSelection(),
          placeholder("搜索本地库"),
          queryLanguage,
          syntaxHighlighting(highlightStyle),
          autocompletion({
            override: [completionSource],
            activateOnTyping: true,
            icons: false,
            maxRenderedOptions: 20,
          }),
          closeBrackets(),
          keymap.of([
            ...closeBracketsKeymap,
            {
              // Tab completes instead of Enter: Enter always runs the query,
              // so a highlighted suggestion can never swallow a submit.
              key: "Tab",
              run: (target) => {
                if (acceptCompletion(target)) return true;
                startCompletion(target);
                return true;
              },
            },
            {
              key: "ArrowDown",
              run: moveCompletionSelection(true),
            },
            {
              key: "ArrowUp",
              run: moveCompletionSelection(false),
            },
            {
              key: "Enter",
              run: () => {
                onsubmit();
                return true;
              },
            },
            {
              key: "Mod-f",
              run: () => {
                onrefine();
                return true;
              },
              preventDefault: true,
            },
            {
              key: "Escape",
              run: (target) => {
                if (closeCompletionCommand(target)) return true;
                target.contentDOM.blur();
                return true;
              },
            },
            ...defaultKeymap,
            ...historyKeymap,
          ]),
          EditorView.updateListener.of((update) => {
            if (!update.docChanged || applying) return;
            onchange(update.state.doc.toString());
          }),
          EditorView.domEventHandlers({
            focus: (_event, target) => {
              onfocus();
              // Empty query: show every queryable key immediately, instead of
              // waiting for the first character. Defer one frame: on a fresh
              // window focus CodeMirror may not have finalized its focus
              // state yet, and starting the completion too early is ignored.
              if (target.state.doc.length === 0) {
                requestAnimationFrame(() => {
                  if (view?.hasFocus && view.state.doc.length === 0) {
                    startCompletion(view);
                  }
                });
              }
            },
            blur: () => onblur(),
          }),
          theme,
        ],
      });
      view = new EditorView({ state, parent: hostEl });
    } catch (e) {
      console.error("code editor init error:", e);
      failure = String(e);
    }
  });

  // Adopt external query changes (startup restore, locate clearing the query,
  // selecting a favorite) without emitting them as user edits.
  $effect(() => {
    const next = value;
    if (!view || applying) return;
    if (view.state.doc.toString() === next) return;
    applying = true;
    view.dispatch({
      changes: { from: 0, to: view.state.doc.length, insert: next },
    });
    applying = false;
  });

  export function focus() {
    view?.focus();
  }

  /** Replace the document and optionally place the caret. */
  export function setValue(next: string, cursor?: number) {
    if (!view) return;
    applying = true;
    view.dispatch({
      changes: { from: 0, to: view.state.doc.length, insert: next },
      selection: cursor != null ? { anchor: Math.min(cursor, next.length) } : undefined,
    });
    applying = false;
    view.focus();
  }

  onDestroy(() => {
    view?.destroy();
    view = null;
  });
</script>

<div class="cm-host" bind:this={hostEl}></div>

{#if failure}
  <!-- Keep the query bar usable if the editor bundle fails to load. -->
  <input
    class="fallback"
    value={value}
    oninput={(e) => onchange((e.currentTarget as HTMLInputElement).value)}
    onfocus={onfocus}
    onblur={onblur}
    onkeydown={(e) => {
      if (e.key === "Enter") {
        e.preventDefault();
        onsubmit();
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "f") {
        e.preventDefault();
        onrefine();
      } else if (e.key === "Escape") {
        (e.currentTarget as HTMLInputElement).blur();
      }
    }}
    spellcheck="false"
    autocomplete="off"
  />
{/if}

<style>
  .cm-host {
    flex: 1;
    min-width: 0;
    overflow: visible;
  }
  .cm-host :global(.cm-editor) {
    height: 28px;
  }
  .fallback {
    flex: 1;
    min-width: 0;
    height: 100%;
    background: none;
    border: none;
    outline: none;
    color: var(--color-base-content);
    font-size: 12.5px;
    user-select: text;
  }
</style>
