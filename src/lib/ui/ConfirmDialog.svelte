<script lang="ts">
  import Dialog from "$lib/ui/Dialog.svelte";

  /** One-step confirmation dialog for destructive list actions. */
  let {
    open = $bindable(false),
    title,
    message = "",
    confirmLabel = "清空",
    cancelLabel = "取消",
    onconfirm,
  }: {
    open?: boolean;
    title: string;
    message?: string;
    confirmLabel?: string;
    cancelLabel?: string;
    onconfirm?: () => void;
  } = $props();
</script>

<Dialog bind:open placement="center" label={title}>
  <div class="confirm">
    <h2>{title}</h2>
    {#if message}<p>{message}</p>{/if}
    <div class="actions">
      <button class="btn" onclick={() => (open = false)}>{cancelLabel}</button>
      <button
        class="btn danger"
        onclick={() => {
          open = false;
          onconfirm?.();
        }}>{confirmLabel}</button>
    </div>
  </div>
</Dialog>

<style>
  .confirm {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 18px;
  }
  h2 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }
  p {
    margin: 0;
    font-size: 12px;
    line-height: 1.5;
    color: color-mix(in oklch, var(--color-base-content) 65%, transparent);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 6px;
  }
  .btn {
    height: 30px;
    padding: 0 14px;
    border-radius: 6px;
    border: 1px solid var(--panel-border);
    background: var(--color-base-100);
    color: var(--color-base-content);
    font-size: 12px;
    cursor: pointer;
  }
  .btn:hover {
    border-color: var(--color-primary);
    color: var(--color-primary);
  }
  .btn.danger {
    border-color: var(--color-error);
    background: var(--color-error);
    color: var(--color-error-content);
  }
  .btn.danger:hover {
    background: color-mix(in oklch, var(--color-error) 85%, black);
    color: var(--color-error-content);
  }
</style>
