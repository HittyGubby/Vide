<script lang="ts">
  import { Settings, Library, Scan } from 'lucide-svelte';
</script>

<div class="settings-view">
  <div class="settings-header">
    <Settings size={18} />
    <h1>设置</h1>
  </div>

  <!-- Library Roots -->
  <section class="setting-group">
    <h2 class="group-title">库管理</h2>
    <p class="group-desc">管理音乐文件的扫描目录</p>
    <div class="setting-card">
      <div class="card-row">
        <span class="card-label">库文件夹</span>
        <button class="btn-secondary">添加</button>
      </div>
      <div class="root-list">
        <div class="root-empty">暂未添加库目录</div>
      </div>
    </div>
    <button class="btn-primary">
      <Scan size={14} />
      <span>立即扫描</span>
    </button>
  </section>

  <!-- About -->
  <section class="setting-group">
    <h2 class="group-title">关于</h2>
    <div class="setting-card">
      <div class="info-row"><span class="info-label">版本</span><span class="info-value">0.1.0</span></div>
      <div class="info-row"><span class="info-label">后端</span><span class="info-value status-ok">运行中</span></div>
    </div>
  </section>
</div>

<style>
  .settings-view {
    max-width: 640px;
    padding: 24px 32px;
    display: flex;
    flex-direction: column;
    gap: 28px;
  }

  .settings-header {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text-primary);
  }

  .settings-header h1 {
    font-size: 18px;
    font-weight: 600;
  }

  .setting-group {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .group-title {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary);
  }

  .group-desc {
    font-size: 11px;
    color: var(--text-tertiary);
    margin-bottom: 4px;
  }

  .setting-card {
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: 8px;
    padding: 12px 16px;
  }

  .card-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .card-label {
    font-size: 13px;
    color: var(--text-primary);
  }

  .root-list {
    margin-top: 8px;
  }

  .root-empty {
    font-size: 12px;
    color: var(--text-tertiary);
    padding: 4px 0;
  }

  .btn-secondary {
    padding: 4px 12px;
    border-radius: 6px;
    background: var(--bg-elevated);
    border: 1px solid var(--border-normal);
    color: var(--text-secondary);
    font-size: 12px;
    transition: background 0.15s;
  }

  .btn-secondary:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .btn-primary {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 8px 16px;
    border-radius: 6px;
    background: var(--accent-primary);
    color: white;
    font-size: 12px;
    font-weight: 500;
    align-self: flex-start;
    transition: background 0.15s;
  }

  .btn-primary:hover {
    background: var(--accent-hover);
  }

  .info-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 4px 0;
  }

  .info-label {
    font-size: 12px;
    color: var(--text-secondary);
  }

  .info-value {
    font-size: 12px;
    color: var(--text-primary);
  }

  .status-ok {
    color: var(--status-success);
  }
</style>
