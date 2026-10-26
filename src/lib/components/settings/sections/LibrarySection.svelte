<script lang="ts">
  import { onMount } from "svelte";
  import { Check, Edit, FolderPlus, Scan, X } from "lucide-svelte";
  import SettingsGroup from "$lib/components/settings/ui/SettingsGroup.svelte";
  import SettingCard from "$lib/components/settings/ui/SettingCard.svelte";
  import { invoke } from "$lib/transport";
  import { isRemoteMode } from "$lib/transport";
  import { isTauri } from "$lib/isTauri";
  import { settingsStore } from "$lib/stores/settings.svelte";

  interface ScanResult {
    scanned: number;
    added: number;
    updated: number;
    skipped: number;
    pruned_paths: number;
    pruned_songs: number;
    errors: string[];
  }

  let roots = $state<string[]>([]);
  let excludeRoots = $state<string[]>([]);
  let newRootPath = $state("");
  let newExcludePath = $state("");
  let showInput = $state(false);
  let showExcludeInput = $state(false);
  let editingInclude = $state<string | null>(null);
  let editingExclude = $state<string | null>(null);
  let scanning = $state(false);
  let scanResult = $state<ScanResult | null>(null);
  let scanFormatsText = $state("");

  async function loadRoots() {
    try {
      roots = await invoke("get_library_roots");
      excludeRoots = await invoke("get_exclude_roots");
    } catch {
      roots = [];
      excludeRoots = [];
    }
  }

  async function saveScanFormats() {
    const values = [
      ...new Set(
        scanFormatsText
          .split(/[\s,]+/)
          .map((value) => value.trim().replace(/^\./, "").toLowerCase())
          .filter(Boolean),
      ),
    ];
    if (values.length === 0) {
      scanFormatsText = settingsStore.settings.scan_extensions.join(" ");
      return;
    }
    scanFormatsText = values.join(" ");
    await settingsStore.update("scan_extensions", values);
  }

  async function addRoot() {
    const path = newRootPath.trim();
    if (!path) return;
    try {
      await invoke("add_library_root", { path });
      await loadRoots();
      newRootPath = "";
      showInput = false;
    } catch (e) {
      console.error("add root error:", e);
    }
  }

  async function removeRoot(path: string) {
    try {
      await invoke("remove_library_root", { path });
      await loadRoots();
    } catch (e) {
      console.error("remove root error:", e);
    }
  }

  function startEditInclude(path: string) {
    editingInclude = path;
    newRootPath = path;
  }

  async function saveEditInclude() {
    if (!editingInclude) return;
    const next = newRootPath.trim();
    if (!next) {
      editingInclude = null;
      return;
    }
    try {
      if (next !== editingInclude) {
        await invoke("remove_library_root", { path: editingInclude });
        await invoke("add_library_root", { path: next });
        await loadRoots();
      }
      editingInclude = null;
      newRootPath = "";
    } catch (e) {
      console.error("edit root error:", e);
    }
  }

  async function addExclude() {
    const path = newExcludePath.trim();
    if (!path) return;
    try {
      await invoke("add_exclude_root", { path });
      await loadRoots();
      newExcludePath = "";
      showExcludeInput = false;
    } catch (e) {
      console.error("add exclude error:", e);
    }
  }

  async function removeExclude(path: string) {
    try {
      await invoke("remove_exclude_root", { path });
      await loadRoots();
    } catch (e) {
      console.error("remove exclude error:", e);
    }
  }

  function startEditExclude(path: string) {
    editingExclude = path;
    newExcludePath = path;
  }

  async function saveEditExclude() {
    if (!editingExclude) return;
    const next = newExcludePath.trim();
    if (!next) {
      editingExclude = null;
      return;
    }
    try {
      if (next !== editingExclude) {
        await invoke("remove_exclude_root", { path: editingExclude });
        await invoke("add_exclude_root", { path: next });
        await loadRoots();
      }
      editingExclude = null;
      newExcludePath = "";
    } catch (e) {
      console.error("edit exclude error:", e);
    }
  }

  async function scanLibrary() {
    scanning = true;
    scanResult = null;
    try {
      scanResult = (await invoke("scan_library")) as ScanResult;
    } catch (e) {
      console.error("scan error:", e);
    } finally {
      scanning = false;
    }
  }

  onMount(async () => {
    await settingsStore.load();
    scanFormatsText = (settingsStore.settings.scan_extensions ?? []).join(" ");
    await loadRoots();
  });
</script>

{#snippet actions()}
  <button
    class="btn-secondary"
    onclick={scanLibrary}
    disabled={scanning || roots.length === 0}
  >
    <Scan size={14} />
    <span>{scanning ? "扫描中..." : "立即扫描"}</span>
  </button>
{/snippet}

<SettingsGroup id="library" title="库管理" actions={actions}>
  <SettingCard>
    <div class="card-head">
      <span class="row-label">包含文件夹</span>
      <button class="btn-secondary" onclick={() => (showInput = !showInput)}>
        <FolderPlus size={13} />
        <span>添加</span>
      </button>
    </div>
    {#if showInput}
      <div class="add-row">
        <input
          type="text"
          class="root-input"
          placeholder="/path/to/music"
          bind:value={newRootPath}
          onkeydown={(e) => {
            if (e.key === "Enter") void addRoot();
          }}
        />
        <button class="btn-add" onclick={() => void addRoot()} disabled={!newRootPath.trim()}>
          确定
        </button>
      </div>
    {/if}
    <div class="root-list">
      {#if roots.length === 0}
        <div class="root-empty">暂未添加包含目录</div>
      {:else}
        {#each roots as root (root)}
          <div class="root-item">
            {#if editingInclude === root}
              <input
                type="text"
                class="root-edit"
                bind:value={newRootPath}
                onkeydown={(e) => {
                  if (e.key === "Enter") void saveEditInclude();
                  if (e.key === "Escape") editingInclude = null;
                }}
              />
              <button class="root-btn ok" onclick={() => void saveEditInclude()} aria-label="保存">
                <Check size={13} />
              </button>
              <button
                class="root-btn"
                onclick={() => {
                  editingInclude = null;
                  newRootPath = "";
                }}
                aria-label="取消"
              >
                <X size={13} />
              </button>
            {:else}
              <span class="root-path" title={root}>{root}</span>
              <button
                class="root-btn"
                onclick={(e) => {
                  e.stopPropagation();
                  startEditInclude(root);
                }}
                aria-label="编辑"
              >
                <Edit size={13} />
              </button>
              <button
                class="root-btn"
                onclick={(e) => {
                  e.stopPropagation();
                  void removeRoot(root);
                }}
                aria-label="删除"
              >
                <X size={13} />
              </button>
            {/if}
          </div>
        {/each}
      {/if}
    </div>
  </SettingCard>

  <SettingCard>
    <div class="card-head">
      <span class="row-label">排除文件夹</span>
      <button class="btn-secondary" onclick={() => (showExcludeInput = !showExcludeInput)}>
        <FolderPlus size={13} />
        <span>添加</span>
      </button>
    </div>
    {#if showExcludeInput}
      <div class="add-row">
        <input
          type="text"
          class="root-input"
          placeholder="/path/to/exclude"
          bind:value={newExcludePath}
          onkeydown={(e) => {
            if (e.key === "Enter") void addExclude();
          }}
        />
        <button class="btn-add" onclick={() => void addExclude()} disabled={!newExcludePath.trim()}>
          确定
        </button>
      </div>
    {/if}
    <div class="root-list">
      {#if excludeRoots.length === 0}
        <div class="root-empty">暂无排除目录</div>
      {:else}
        {#each excludeRoots as path (path)}
          <div class="root-item">
            {#if editingExclude === path}
              <input
                type="text"
                class="root-edit"
                bind:value={newExcludePath}
                onkeydown={(e) => {
                  if (e.key === "Enter") void saveEditExclude();
                  if (e.key === "Escape") editingExclude = null;
                }}
              />
              <button class="root-btn ok" onclick={() => void saveEditExclude()} aria-label="保存">
                <Check size={13} />
              </button>
              <button
                class="root-btn"
                onclick={() => {
                  editingExclude = null;
                  newExcludePath = "";
                }}
                aria-label="取消"
              >
                <X size={13} />
              </button>
            {:else}
              <span class="root-path" title={path}>{path}</span>
              <button
                class="root-btn"
                onclick={(e) => {
                  e.stopPropagation();
                  startEditExclude(path);
                }}
                aria-label="编辑"
              >
                <Edit size={13} />
              </button>
              <button
                class="root-btn"
                onclick={(e) => {
                  e.stopPropagation();
                  void removeExclude(path);
                }}
                aria-label="删除"
              >
                <X size={13} />
              </button>
            {/if}
          </div>
        {/each}
      {/if}
    </div>
  </SettingCard>

  {#if isTauri && !isRemoteMode()}
    <SettingCard>
      <div class="card-head">
        <span class="row-label">扫描格式</span>
      </div>
      <p class="hint">空格分隔，可写 .mp3 或 mp3；MIME 只用于 HTTP 输出，不用来扫描。</p>
      <input
        class="format-input"
        type="text"
        spellcheck="false"
        placeholder="mp3 flac ogg wav m4a"
        bind:value={scanFormatsText}
        onblur={() => void saveScanFormats()}
        onkeydown={(e) => {
          if (e.key === "Enter") e.currentTarget.blur();
        }}
      />
    </SettingCard>
  {/if}

  {#if scanResult}
    <p class="scan-result">
      扫描完成: {scanResult.scanned} 个文件, 新增 {scanResult.added} 首,
      更新 {scanResult.updated} 首, 跳过 {scanResult.skipped} 首
      {#if scanResult.pruned_paths > 0 || scanResult.pruned_songs > 0}
        · 清理 {scanResult.pruned_paths} 个失效路径、{scanResult.pruned_songs} 首空歌曲
      {/if}
      {#if scanResult.errors.length > 0} · {scanResult.errors.length} 个错误{/if}
    </p>
  {/if}
</SettingsGroup>

<style>
  .card-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 4px 0;
  }
  .row-label {
    font-size: 13px;
    color: var(--color-base-content);
  }
  .btn-secondary {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 5px 12px;
    border-radius: 6px;
    background: var(--color-base-300);
    color: var(--color-base-content);
    font-size: 12px;
    border: none;
    cursor: pointer;
  }
  .btn-secondary:hover {
    background: var(--color-base-100);
  }
  .btn-secondary:disabled {
    opacity: 0.5;
  }

  .add-row {
    display: flex;
    gap: 6px;
    padding: 4px 0;
  }
  .root-input {
    flex: 1;
    min-width: 0;
    padding: 6px 10px;
    border-radius: 6px;
    background: var(--color-base-100);
    border: 1px solid var(--panel-border);
    color: var(--color-base-content);
    font-size: 12px;
    user-select: text;
  }
  .btn-add {
    flex: none;
    padding: 6px 14px;
    border-radius: 6px;
    background: var(--color-primary);
    color: var(--color-primary-content);
    font-size: 12px;
    border: none;
    cursor: pointer;
  }
  .btn-add:disabled {
    opacity: 0.5;
  }

  .root-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 4px 0;
  }
  .root-empty {
    font-size: 12px;
    padding: 4px 0;
    color: color-mix(in oklch, var(--color-base-content) 50%, transparent);
  }
  .root-item {
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 30px;
    font-size: 12px;
    font-family: ui-monospace, "SFMono-Regular", Menlo, Consolas, monospace;
    color: color-mix(in oklch, var(--color-base-content) 85%, transparent);
    user-select: text;
  }
  .root-path {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .root-btn {
    flex: none;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border-radius: 5px;
    background: none;
    border: none;
    color: color-mix(in oklch, var(--color-base-content) 55%, transparent);
    cursor: pointer;
  }
  .root-btn:hover {
    background: var(--color-base-300);
    color: var(--color-base-content);
  }
  .root-btn.ok {
    color: var(--color-success);
  }
  .root-edit {
    flex: 1;
    min-width: 0;
    height: 28px;
    padding: 0 8px;
    background: var(--color-base-100);
    border: 1px solid var(--panel-border);
    border-radius: 5px;
    color: var(--color-base-content);
    font-size: 12px;
    font-family: ui-monospace, "SFMono-Regular", Menlo, Consolas, monospace;
    user-select: text;
  }

  .format-input {
    width: 100%;
    box-sizing: border-box;
    padding: 7px 9px;
    border: 1px solid var(--color-base-300);
    border-radius: 6px;
    background: var(--color-base-100);
    color: var(--color-base-content);
    font: 12px ui-monospace, "SFMono-Regular", Menlo, Consolas, monospace;
  }
  .format-input:focus {
    outline: none;
    border-color: var(--color-primary);
  }

  .hint {
    margin: 0;
    font-size: 11px;
    line-height: 1.5;
    color: color-mix(in oklch, var(--color-base-content) 55%, transparent);
  }
  .scan-result {
    margin: 0;
    padding: 4px 0;
    font-size: 11px;
    line-height: 1.6;
    color: color-mix(in oklch, var(--color-base-content) 70%, transparent);
  }
</style>
