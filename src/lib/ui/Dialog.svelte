<script lang="ts">
  import type { Snippet } from "svelte";

  /**
   * Thin wrapper around the native `<dialog>` element.
   *
   * Native dialogs give us the focus trap, Escape handling, the top layer and
   * the inert backdrop for free — none of the hand-rolled overlays in this app
   * have those. `placement` only changes the box position: bottom sheets on
   * phones, left drawers for navigation, centered dialogs for forms.
   */
  let {
    open = $bindable(false),
    placement = "center",
    label = "",
    onclose,
    children,
  }: {
    open?: boolean;
    placement?: "center" | "bottom" | "left";
    label?: string;
    onclose?: () => void;
    children: Snippet;
  } = $props();

  let el = $state<HTMLDialogElement | null>(null);

  $effect(() => {
    const dialog = el;
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    else if (!open && dialog.open) dialog.close();
  });

  function handleClose() {
    open = false;
    onclose?.();
  }
</script>

<dialog
  bind:this={el}
  class="ui-dialog {placement}"
  aria-label={label || undefined}
  onclose={handleClose}
  onclick={(event) => {
    if (event.target === el) handleClose();
  }}
>
  <div class="ui-dialog-body">
    {@render children()}
  </div>
</dialog>

<style>
  .ui-dialog {
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--color-base-content);
    max-width: 100vw;
    max-height: 100dvh;
  }

  .ui-dialog::backdrop {
    background: rgba(0, 0, 0, 0.55);
  }

  .ui-dialog-body {
    background: var(--color-base-200);
    color: var(--color-base-content);
    border: 1px solid var(--panel-border-strong);
    box-shadow: var(--shadow-lg);
    overflow: auto;
    overscroll-behavior: contain;
  }

  /* ── Center: forms and confirmations ─────────────── */
  .ui-dialog.center {
    margin: auto;
  }
  .ui-dialog.center .ui-dialog-body {
    width: min(420px, calc(100vw - 32px));
    max-height: calc(100dvh - 32px);
    border-radius: 12px;
  }

  /* ── Bottom sheet: row actions, pickers ──────────── */
  .ui-dialog.bottom {
    margin: auto 0 0 0;
    width: 100%;
  }
  .ui-dialog.bottom .ui-dialog-body {
    width: 100%;
    max-height: 80dvh;
    border-width: 1px 0 0;
    border-radius: 14px 14px 0 0;
    padding-bottom: env(safe-area-inset-bottom);
    animation: sheet-in 0.18s ease-out;
  }
  @keyframes sheet-in {
    from {
      transform: translateY(18px);
      opacity: 0.5;
    }
  }

  /* ── Left drawer: navigation ─────────────────────── */
  .ui-dialog.left {
    margin: 0 auto 0 0;
    height: 100dvh;
  }
  .ui-dialog.left .ui-dialog-body {
    width: min(300px, 84vw);
    height: 100%;
    border-width: 0 1px 0 0;
    border-radius: 0;
    animation: drawer-in 0.18s ease-out;
  }
  @keyframes drawer-in {
    from {
      transform: translateX(-16px);
      opacity: 0.6;
    }
  }
</style>
