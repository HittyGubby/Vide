<script lang="ts">
  import { onMount } from "svelte";
  import { Link, LogOut, Loader } from "lucide-svelte";
  import {
    connectRemote,
    disconnectRemote,
    isRemoteMode,
    remoteSession,
    getPlaybackTarget,
    setPlaybackTarget,
    isRemoteGuest,
    type PlaybackTarget,
    type RemoteSession,
  } from "$lib/transport";

  let baseUrl = $state("");
  let password = $state("");
  let busy = $state(false);
  let error = $state<string | null>(null);
  let current = $state<RemoteSession | null>(null);
  let target = $state<PlaybackTarget>("server");
  let loginPassword = $state("");
  let loginBusy = $state(false);

  onMount(() => {
    current = remoteSession();
    baseUrl = current?.baseUrl ?? "http://127.0.0.1:8080";
    target = getPlaybackTarget();
  });

  function chooseTarget(next: PlaybackTarget) {
    target = next;
    setPlaybackTarget(next);
  }

  async function connect() {
    busy = true;
    error = null;
    try {
      current = await connectRemote(baseUrl, password);
      password = "";
      window.location.reload();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function loginRemote() {
    if (!current) return;
    loginBusy = true;
    error = null;
    try {
      current = await connectRemote(current.baseUrl, loginPassword);
      loginPassword = "";
      window.location.reload();
    } catch (e) {
      error = String(e);
    } finally {
      loginBusy = false;
    }
  }

  function disconnect() {
    disconnectRemote();
    current = null;
    window.location.reload();
  }
</script>

<section class="group">
  <h2 class="group-title">远程连接</h2>
  <div class="setting-card remote-card">
    {#if isRemoteMode() && current}
      <div class="remote-current">
        <span class="dot"></span>
        <span class="remote-url">{current.baseUrl}</span>
        <button class="btn-secondary" onclick={disconnect}><LogOut size={13} />断开并回到本地</button>
      </div>
      <p class="hint">当前所有数据与播放请求都走远程服务端。本地库、插件和缓存设置不适用于远程模式。</p>
      {#if isRemoteGuest()}
        <p class="hint">当前为游客远程连接：只能只读。登录后才能播放控制、编辑和修改服务端设置。</p>
        <div class="remote-form">
          <input type="password" bind:value={loginPassword} placeholder="服务端密码" onkeydown={(e) => { if (e.key === "Enter") void loginRemote(); }} />
          <button class="btn-secondary" onclick={() => void loginRemote()} disabled={loginBusy || !loginPassword}>
            {#if loginBusy}<Loader size={13} class="spin" />{:else}登录{/if}
          </button>
        </div>
      {:else}
        <div class="playback-target">
          <span>播放位置</span>
          <label><input type="radio" name="playback-target" checked={target === "server"} onchange={() => chooseTarget("server")} /> 服务端播放</label>
          <label><input type="radio" name="playback-target" checked={target === "client"} onchange={() => chooseTarget("client")} /> 客户端播放</label>
        </div>
      {/if}
    {:else}
      <p class="hint">连接 vide-server 后，桌面端可以作为远程客户端使用。先在本机运行服务端并设置密码。</p>
      <div class="remote-form">
        <input type="text" bind:value={baseUrl} placeholder="http://127.0.0.1:8080" spellcheck="false" />
        <input type="password" bind:value={password} placeholder="服务端密码（留空为游客连接）" onkeydown={(e) => { if (e.key === "Enter") void connect(); }} />
        <button class="btn-secondary" onclick={() => void connect()} disabled={busy || !baseUrl.trim()}>
          {#if busy}<Loader size={13} class="spin" />{:else}<Link size={13} />{/if}
          连接
        </button>
      </div>
      {#if error}<p class="error">{error}</p>{/if}
    {/if}
  </div>
</section>

<style>
  .group { display: flex; flex-direction: column; gap: 8px; }
  .group-title { font-size: 14px; font-weight: 700; color: var(--color-primary); padding-bottom: 6px; border-bottom: 1px solid var(--panel-border); }
  .setting-card { background: var(--color-base-200); border: 1px solid var(--panel-border); border-radius: 8px; padding: 10px 14px; display: flex; flex-direction: column; gap: 8px; }
  .remote-form { display: flex; gap: 6px; flex-wrap: wrap; }
  .remote-form input { flex: 1 1 220px; padding: 6px 9px; border: 1px solid var(--color-base-300); border-radius: 5px; background: var(--color-base-100); color: inherit; font: inherit; }
  .remote-form input:focus { outline: none; border-color: var(--color-primary); }
  .remote-current { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; font-size: 12px; }
  .playback-target { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; font-size: 11px; color: var(--color-base-content) / 0.7; }
  .playback-target label { display: inline-flex; align-items: center; gap: 4px; }
  .playback-target input { accent-color: var(--color-primary); }
  .dot { width: 8px; height: 8px; border-radius: 50%; background: var(--color-success); }
  .remote-url { font-family: var(--font-mono, monospace); font-size: 11px; color: var(--color-base-content) / 0.75; }
  .btn-secondary { display: inline-flex; align-items: center; gap: 4px; padding: 4px 9px; border: 1px solid var(--panel-border); border-radius: 5px; background: var(--color-base-100); color: inherit; font: inherit; font-size: 11px; cursor: pointer; }
  .btn-secondary:hover:not(:disabled) { border-color: var(--color-primary); color: var(--color-primary); }
  .btn-secondary:disabled { opacity: 0.5; }
  .hint { margin: 0; font-size: 11px; line-height: 1.5; color: var(--color-base-content) / 0.55; }
  .error { margin: 0; font-size: 11px; color: var(--color-error); overflow-wrap: anywhere; }
  .spin { animation: spin 0.8s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
