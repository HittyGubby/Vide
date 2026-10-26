<script lang="ts">
  let {
    label,
    checked,
    onchange,
  }: {
    label: string;
    checked: boolean;
    onchange: (checked: boolean) => void;
  } = $props();
</script>

<label class="setting-toggle">
  <span>{label}</span>
  <input
    type="checkbox"
    checked={checked}
    onchange={(e) => onchange(e.currentTarget.checked)}
  />
</label>

<style>
  .setting-toggle {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    padding: 9px 0;
    min-height: 36px;
    font-size: 13px;
    color: var(--color-base-content);
    cursor: pointer;
  }

  .setting-toggle + :global(.setting-toggle) {
    border-top: 1px solid var(--panel-border);
  }

  span {
    min-width: 0;
  }

  input {
    flex: none;
    width: 18px;
    height: 18px;
    accent-color: var(--color-primary);
  }
</style>
