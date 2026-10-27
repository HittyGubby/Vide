<script lang="ts">
  import { Loader } from "lucide-svelte";
  import Dialog from "$lib/ui/Dialog.svelte";
  import { browserLogin } from "$lib/transport";
  import { auth } from "$lib/stores/auth.svelte";
  import { clearGuestSettings } from "$lib/themeGuest";

  let { open = $bindable(false) }: { open?: boolean } = $props();

  let password = $state("");
  let busy = $state(false);
  let error = $state<string | null>(null);

  async function login() {
    if (!password || busy) return;
    busy = true;
    error = null;
    try {
      await browserLogin(password);
      auth.authenticated = true;
      // From now on the server's settings own the theme.
      clearGuestSettings();
      password = "";
      open = false;
      window.location.reload();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<Dialog bind:open placement="center" label="登录 Vide Server">
  <form
    class="login-panel"
    onsubmit={(e) => {
      e.preventDefault();
      void login();
    }}
  >
    <h2>登录 Vide Server</h2>
    <p>只读浏览无需登录；播放控制、标签和数据修改需要登录。</p>
    <input
      type="password"
      bind:value={password}
      placeholder="密码"
      autocomplete="current-password"
    />
    {#if error}<div class="login-error">{error}</div>{/if}
    <div class="login-actions">
      <button type="button" onclick={() => (open = false)}>取消</button>
      <button type="submit" class="primary" disabled={busy || !password}>
        {#if busy}<Loader size={13} class="spin" />{:else}登录{/if}
      </button>
    </div>
  </form>
</Dialog>

<style>
  .login-panel {
    display: flex;
    flex-direction: column;
    gap: 9px;
    padding: 18px;
  }
  h2 {
    margin: 0;
    font-size: 15px;
  }
  p {
    margin: 0;
    font-size: 11px;
    line-height: 1.5;
    color: color-mix(in oklch, var(--color-base-content) 55%, transparent);
  }
  input {
    padding: 7px 9px;
    border: 1px solid var(--panel-border);
    border-radius: 5px;
    background: var(--color-base-100);
    color: inherit;
    font: inherit;
  }
  input:focus {
    outline: none;
    border-color: var(--color-primary);
  }
  .login-error {
    color: var(--color-error);
    font-size: 11px;
  }
  .login-actions {
    display: flex;
    justify-content: flex-end;
    gap: 6px;
  }
  .login-actions button {
    padding: 4px 9px;
    border: 1px solid var(--panel-border);
    border-radius: 5px;
    background: var(--color-base-200);
    color: inherit;
    cursor: pointer;
  }
  .login-actions .primary {
    border-color: var(--color-primary);
    color: var(--color-primary);
  }
  :global(.spin) {
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
