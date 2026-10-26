<script lang="ts">
  import { ChevronDown, ChevronRight } from "lucide-svelte";
  import type { PluginInfo } from "$lib/types";
  import {
    capabilityGroups,
    inheritedDefault,
    platformPlaceholder,
    pluginKey,
  } from "$lib/components/settings/pluginHelpers";

  /** One installed plugin: enable toggle, capabilities, and editable argv. */
  let {
    plugin,
    expanded,
    argvDrafts,
    platformDrafts,
    concurrencyDrafts,
    busy,
    ontoggleenabled,
    ontoggleexpand,
    onsaveargv,
    onresetargv,
    onsaveplatform,
    onsaveconcurrency,
  }: {
    plugin: PluginInfo;
    expanded: boolean;
    argvDrafts: Record<string, string>;
    platformDrafts: Record<string, string>;
    concurrencyDrafts: Record<string, string>;
    busy: string | null;
    ontoggleenabled: (plugin: PluginInfo, enabled: boolean) => void;
    ontoggleexpand: (plugin: PluginInfo) => void;
    onsaveargv: (plugin: PluginInfo) => void;
    onresetargv: (plugin: PluginInfo) => void;
    onsaveplatform: (plugin: PluginInfo, platform: string) => void;
    onsaveconcurrency: (plugin: PluginInfo) => void;
  } = $props();
</script>

<div class="plugin">
  <div class="plugin-head">
    <label class="check" title={plugin.enabled ? "禁用插件" : "启用插件"}>
      <input
        type="checkbox"
        checked={plugin.enabled}
        onchange={(e) => ontoggleenabled(plugin, e.currentTarget.checked)}
      />
    </label>
    <button class="plugin-toggle" onclick={() => ontoggleexpand(plugin)}>
      {#if expanded}<ChevronDown size={14} />{:else}<ChevronRight size={14} />{/if}
      <span class="plugin-name" class:muted={!plugin.enabled}>{plugin.name}</span>
      <span class="version">v{plugin.version}</span>
      {#if plugin.description}
        <span class="desc">{plugin.description}</span>
      {/if}
    </button>
  </div>

  <div class="cap-row">
    {#each capabilityGroups(plugin) as group (group.platform)}
      <span class="cap-chip" title={group.platform + ": " + group.labels.join(", ")}>
        <span class="cap-platform">{group.platform}</span>
        <span class="cap-value">{group.all ? "全部" : group.labels.join(" / ")}</span>
      </span>
    {/each}
  </div>

  {#if expanded}
    <div class="plugin-detail">
      <p class="dir">{plugin.dir}</p>

      <div class="field">
        <div class="field-head">
          <span>默认 argv</span>
          <div class="field-actions">
            <button
              class="btn-mini"
              onclick={() => onresetargv(plugin)}
              disabled={busy !== null}
            >恢复清单默认</button>
            <button
              class="btn-mini primary"
              onclick={() => onsaveargv(plugin)}
              disabled={busy !== null}
            >保存</button>
          </div>
        </div>
        <textarea
          rows="3"
          spellcheck="false"
          placeholder={inheritedDefault(plugin)}
          value={argvDrafts[plugin.name] ?? ""}
          oninput={(e) => (argvDrafts[plugin.name] = e.currentTarget.value)}
        ></textarea>
        <p class="hint">每行一个参数；留空则继承清单 exec。</p>
      </div>

      {#each Array.from(new Set(plugin.capabilities.map((capability) => capability.platform))) as platform (platform)}
        {@const key = pluginKey(plugin.name, platform)}
        <div class="field">
          <div class="field-head">
            <span>{platform} argv 覆盖</span>
            <button
              class="btn-mini"
              onclick={() => onsaveplatform(plugin, platform)}
              disabled={busy !== null}
            >保存</button>
          </div>
          <textarea
            rows="2"
            spellcheck="false"
            placeholder={platformPlaceholder(plugin)}
            value={platformDrafts[key] ?? ""}
            oninput={(e) => (platformDrafts[key] = e.currentTarget.value)}
          ></textarea>
        </div>
      {/each}

      <div class="field">
        <div class="field-head">
          <span>并发上限</span>
          <button
            class="btn-mini"
            onclick={() => onsaveconcurrency(plugin)}
            disabled={busy !== null}
          >保存</button>
        </div>
        <input
          class="text-input"
          type="number"
          min="1"
          step="1"
          placeholder="默认"
          value={concurrencyDrafts[plugin.name] ?? ""}
          oninput={(e) => (concurrencyDrafts[plugin.name] = e.currentTarget.value)}
        />
      </div>

      {#if plugin.last_stderr}
        <div class="field">
          <span class="field-label">最近一次 stderr</span>
          <pre class="stderr">{plugin.last_stderr}</pre>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .plugin {
    border: 1px solid var(--panel-border);
    border-radius: 7px;
    padding: 6px 8px;
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .plugin-head {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }
  .plugin-toggle {
    display: flex;
    align-items: center;
    gap: 5px;
    flex: 1;
    min-width: 0;
    padding: 0;
    border: 0;
    background: none;
    color: var(--color-base-content);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .plugin-name {
    font-size: 13px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .version {
    flex: none;
    font-size: 10px;
    color: color-mix(in oklch, var(--color-base-content) 45%, transparent);
  }
  .desc {
    flex: 1;
    min-width: 0;
    font-size: 10.5px;
    color: color-mix(in oklch, var(--color-base-content) 50%, transparent);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .check {
    display: flex;
    align-items: center;
    flex: none;
    cursor: pointer;
  }
  .check input {
    width: 14px;
    height: 14px;
    accent-color: var(--color-primary);
  }
  .muted {
    color: color-mix(in oklch, var(--color-base-content) 40%, transparent);
  }
  .cap-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
    min-width: 0;
  }
  .cap-chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 10px;
    padding: 1px 6px;
    border-radius: 4px;
    color: color-mix(in oklch, var(--color-base-content) 70%, transparent);
    background: var(--color-base-100);
    border: 1px solid var(--panel-border);
  }
  .cap-platform {
    color: var(--color-primary);
    font-weight: 600;
  }
  .cap-value {
    color: color-mix(in oklch, var(--color-base-content) 55%, transparent);
  }
  .plugin-detail {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-top: 4px;
    padding-top: 8px;
    border-top: 1px solid var(--panel-border);
  }
  .dir {
    margin: 0;
    font-size: 10px;
    color: color-mix(in oklch, var(--color-base-content) 45%, transparent);
    font-family: ui-monospace, "SFMono-Regular", Menlo, Consolas, monospace;
    overflow-wrap: anywhere;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .field-label {
    font-size: 11px;
    color: color-mix(in oklch, var(--color-base-content) 60%, transparent);
  }
  .field-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    font-size: 11px;
    color: color-mix(in oklch, var(--color-base-content) 70%, transparent);
  }
  .field-actions {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  textarea,
  .text-input {
    width: 100%;
    box-sizing: border-box;
    font: 11px ui-monospace, "SFMono-Regular", Menlo, Consolas, monospace;
    line-height: 1.4;
    color: var(--color-base-content);
    background: var(--color-base-100);
    border: 1px solid var(--panel-border);
    border-radius: 5px;
    padding: 5px 7px;
    resize: vertical;
  }
  textarea:focus,
  .text-input:focus {
    outline: none;
    border-color: var(--color-primary);
  }
  .btn-mini {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    padding: 2px 7px;
    border-radius: 4px;
    border: 1px solid var(--panel-border);
    background: var(--color-base-100);
    color: color-mix(in oklch, var(--color-base-content) 80%, transparent);
    font-size: 10.5px;
    line-height: 1.5;
    font-family: inherit;
    cursor: pointer;
  }
  .btn-mini:hover:not(:disabled) {
    border-color: var(--color-primary);
    color: var(--color-primary);
  }
  .btn-mini:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .btn-mini.primary {
    border-color: color-mix(in oklch, var(--color-primary) 45%, transparent);
    color: var(--color-primary);
  }
  .hint {
    margin: 0;
    font-size: 10.5px;
    line-height: 1.5;
    color: color-mix(in oklch, var(--color-base-content) 50%, transparent);
  }
  .stderr {
    margin: 0;
    max-height: 140px;
    overflow: auto;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    font: 10px ui-monospace, "SFMono-Regular", Menlo, Consolas, monospace;
    color: color-mix(in oklch, var(--color-base-content) 65%, transparent);
    background: var(--color-base-100);
    border: 1px solid var(--panel-border);
    border-radius: 4px;
    padding: 5px 6px;
  }
</style>
