<script lang="ts">
  import { searchPresets } from "$lib/stores/searchPresets.svelte";
  import { Plus, X } from "lucide-svelte";
  import type { Song } from "$lib/types";

  let {
    activeId,
    onSelect,
    onPlay,
    songs,
  }: {
    activeId: string | null;
    onSelect: (id: string | null) => void;
    onPlay: (song: Song, idx: number) => void;
    songs: Song[];
  } = $props();

  let showModal = $state(false);
  let newName = $state("");
  let newQuery = $state("");

  function openModal() {
    newName = "";
    newQuery = "";
    showModal = true;
  }

  function closeModal() {
    showModal = false;
  }

  async function addPreset() {
    if (!newName.trim() || !newQuery.trim()) return;
    await searchPresets.save(newName.trim(), newQuery.trim());
    closeModal();
  }
</script>

<div class="preset-panel">
  <div class="panel-header">
    <h2 class="panel-title">搜索预设</h2>
    <div class="header-btns">
      <button class="h-btn" title="新建预设" onclick={openModal} aria-label="新建">
        <Plus size={15} />
      </button>
    </div>
  </div>

  <div class="panel-list">
    <button class="panel-item" class:active={activeId === null} onclick={() => onSelect(null)}>
      <span class="item-label">全部歌曲</span>
      <span class="item-count">{songs.length}</span>
    </button>

    {#each searchPresets.presets as preset}
      <button
        class="panel-item"
        class:active={activeId === preset.id}
        onclick={() => onSelect(preset.id)}
        oncontextmenu={(e) => e.preventDefault()}
      >
        <span class="chev">&#9656;</span>
        <span class="item-label">{preset.name}</span>
      </button>
    {/each}
  </div>
</div>

{#if showModal}
  <div class="modal-backdrop" onclick={closeModal} role="presentation"></div>
  <div class="preset-modal" role="dialog" aria-label="新建搜索预设">
    <div class="modal-header">
      <span class="modal-title">新建搜索预设</span>
      <button class="modal-close" onclick={closeModal} aria-label="关闭">
        <X size={15} />
      </button>
    </div>
    <div class="modal-body">
      <label class="field">
        <span>名称</span>
        <input type="text" placeholder="如：俄罗斯民谣" bind:value={newName} autofocus />
      </label>
      <label class="field">
        <span>查询</span>
        <input
          type="text"
          placeholder="如：country:russia AND genre:folk"
          bind:value={newQuery}
          onkeydown={(e) => { if (e.key === "Enter") addPreset(); }}
        />
      </label>
      <p class="modal-hint">查询用结构化语法构建动态视图</p>
      <div class="modal-actions">
        <button class="btn-cancel" onclick={closeModal}>取消</button>
        <button class="btn-save" onclick={addPreset} disabled={!newName.trim() || !newQuery.trim()}>
          保存
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .preset-panel {
    width: 180px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    background: var(--color-base-200);
    border-right: 1px solid var(--color-base-300);
    min-height: 0;
  }

  .panel-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 10px;
    height: 36px;
    border-bottom: 1px solid var(--color-base-300);
  }

  .panel-title { font-size: 12px; color: var(--color-base-content); }

  .h-btn {
    display: flex; align-items: center; justify-content: center;
    width: 26px; height: 26px; border-radius: 5px;
    background: none; border: none; color: var(--color-base-content) / 0.7;
  }
  .h-btn:hover { background: var(--color-base-300); color: var(--color-base-content); }

  .panel-list { flex: 1; overflow-y: auto; padding: 4px 0; }

  .panel-item {
    display: flex; align-items: center; gap: 4px;
    width: 100%; height: 34px; padding: 0 10px;
    font-size: 13px; text-align: left;
    color: var(--color-base-content) / 0.9;
    background: none; border: none; cursor: pointer;
    transition: background 0.12s;
  }
  .panel-item:hover { background: var(--color-base-300); }
  .panel-item.active { color: var(--color-primary); background: var(--color-primary) / 0.12; }

  .chev { font-size: 9px; opacity: 0.5; }
  .item-label { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .item-count { font-size: 11px; color: var(--color-base-content) / 0.5; }

  .modal-backdrop {
    position: fixed; inset: 0; z-index: 500;
    background: rgba(0, 0, 0, 0.5);
  }

  .preset-modal {
    position: fixed; z-index: 501;
    top: 50%; left: 50%; transform: translate(-50%, -50%);
    width: 400px; max-width: 90vw;
    background: var(--color-base-300);
    border: 1px solid var(--color-base-content) / 0.1;
    border-radius: 10px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
    display: flex; flex-direction: column; overflow: hidden;
  }

  .modal-header {
    display: flex; align-items: center; justify-content: space-between;
    padding: 14px 16px 10px;
  }
  .modal-title { font-size: 14px; font-weight: 600; color: var(--color-base-content); }
  .modal-close {
    display: flex; align-items: center; justify-content: center;
    width: 28px; height: 28px; border-radius: 6px;
    color: var(--color-base-content) / 0.7; background: none; border: none;
  }
  .modal-close:hover { background: var(--color-base-200); color: var(--color-base-content); }

  .modal-body {
    padding: 4px 16px 16px;
    display: flex; flex-direction: column; gap: 12px;
  }

  .field { display: flex; flex-direction: column; gap: 5px; }
  .field span { font-size: 11px; color: var(--color-base-content) / 0.6; }
  .field input {
    height: 34px; padding: 0 10px; border-radius: 6px;
    background: var(--color-base-200);
    border: 1px solid var(--color-base-content) / 0.1;
    color: var(--color-base-content); font-size: 13px;
    user-select: text; outline: none;
  }
  .field input:focus { border-color: var(--color-primary); }

  .modal-hint { font-size: 11px; color: var(--color-base-content) / 0.5; }

  .modal-actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 4px; }
  .btn-cancel {
    padding: 7px 14px; border-radius: 6px;
    background: var(--color-base-200);
    color: var(--color-base-content) / 0.85; border: none; font-size: 12px;
  }
  .btn-cancel:hover { background: var(--color-base-100); }
  .btn-save {
    padding: 7px 16px; border-radius: 6px;
    background: var(--color-primary);
    color: var(--color-primary-content); border: none; font-size: 12px;
  }
  .btn-save:disabled { opacity: 0.5; }
</style>
