<script lang="ts">
  import { onMount, tick, type Component } from "svelte";
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import {
    ArrowLeft,
    ChevronRight,
    Database,
    Folder,
    HardDrive,
    Keyboard,
    MessageSquareQuote,
    Palette,
    Play,
    Plug,
    Server,
    Settings as SettingsIcon,
    SlidersHorizontal,
    Type as TypeIcon,
  } from "lucide-svelte";
  import { platform } from "$lib/platform.svelte";
  import { isTauri } from "$lib/isTauri";
  import { canManage } from "$lib/stores/auth.svelte";
  import { settingsStore } from "$lib/stores/settings.svelte";

  import RemoteSection from "$lib/components/settings/sections/RemoteSection.svelte";
  import GeneralSection from "$lib/components/settings/sections/GeneralSection.svelte";
  import AppearanceSection from "$lib/components/settings/sections/AppearanceSection.svelte";
  import PlaybackSection from "$lib/components/settings/sections/PlaybackSection.svelte";
  import ShortcutsSection from "$lib/components/settings/sections/ShortcutsSection.svelte";
  import CacheSection from "$lib/components/settings/sections/CacheSection.svelte";
  import DesktopLyricsSection from "$lib/components/settings/sections/DesktopLyricsSection.svelte";
  import LyricsStyleSection from "$lib/components/settings/sections/LyricsStyleSection.svelte";
  import LibrarySection from "$lib/components/settings/sections/LibrarySection.svelte";
  import PluginsSection from "$lib/components/settings/sections/PluginsSection.svelte";
  import DataSection from "$lib/components/settings/sections/DataSection.svelte";

  interface Section {
    id: string;
    label: string;
    /** Any lucide icon component. */
    icon: typeof Server;
    /** Whether the current runtime/user may see this section. */
    available: () => boolean;
    component: Component;
  }

  const SECTIONS: Section[] = [
    { id: "remote", label: "远程连接", icon: Server, available: () => isTauri, component: RemoteSection },
    { id: "general", label: "通用", icon: SlidersHorizontal, available: () => isTauri, component: GeneralSection },
    { id: "appearance", label: "外观", icon: Palette, available: () => true, component: AppearanceSection },
    { id: "playback", label: "播放", icon: Play, available: () => true, component: PlaybackSection },
    { id: "shortcuts", label: "快捷键", icon: Keyboard, available: () => isTauri, component: ShortcutsSection },
    { id: "cache", label: "缓存管理", icon: HardDrive, available: () => canManage(), component: CacheSection },
    { id: "desktop-lyrics", label: "桌面歌词", icon: MessageSquareQuote, available: () => isTauri, component: DesktopLyricsSection },
    { id: "lyrics-style", label: "歌词样式", icon: TypeIcon, available: () => true, component: LyricsStyleSection },
    { id: "library", label: "库管理", icon: Folder, available: () => canManage(), component: LibrarySection },
    { id: "plugins", label: "插件", icon: Plug, available: () => canManage(), component: PluginsSection },
    { id: "data", label: "数据", icon: Database, available: () => true, component: DataSection },
  ];

  let visibleSections = $derived(SECTIONS.filter((section) => section.available()));
  let activeId = $derived(String($page.url.searchParams.get("section") ?? ""));
  let activeSection = $derived(
    visibleSections.find((section) => section.id === activeId) ?? null,
  );

  // Desktop anchor navigation highlight.
  let navActive = $state("");

  onMount(async () => {
    await settingsStore.load();
    navActive = visibleSections[0]?.id ?? "";
    if (!platform.mobileUI && activeId) {
      await tick();
      scrollToSection(activeId, "auto");
    }
  });

  function openSection(id: string) {
    void goto("/settings?section=" + id);
  }

  /** Replace the section entry so Back leaves settings instead of reopening it. */
  function closeSection() {
    void goto("/settings", { replaceState: true });
  }

  function scrollToSection(id: string, behavior: ScrollBehavior = "smooth") {
    const el = document.getElementById("section-" + id);
    if (!el) return;
    el.scrollIntoView({ behavior, block: "start" });
    navActive = id;
  }

  // Cheap scroll-spy for the desktop anchor bar.
  $effect(() => {
    if (platform.mobileUI) return;
    const els = visibleSections
      .map((section) => document.getElementById("section-" + section.id))
      .filter((el): el is HTMLElement => el !== null);
    if (els.length === 0) return;
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            navActive = entry.target.id.replace("section-", "");
          }
        }
      },
      { rootMargin: "-25% 0px -65% 0px", threshold: 0 },
    );
    for (const el of els) observer.observe(el);
    return () => observer.disconnect();
  });
</script>

<div class="settings-view" class:mobile={platform.mobileUI}>
  {#if platform.mobileUI}
    {#if activeSection}
      {@const Section = activeSection.component}
      <div class="section-head">
        <button class="back-btn" onclick={closeSection} aria-label="返回设置">
          <ArrowLeft size={18} />
        </button>
        <h1>{activeSection.label}</h1>
      </div>
      <div class="mobile-sections">
        <Section />
      </div>
    {:else}
      <div class="category-list">
        {#each visibleSections as section (section.id)}
          <button class="category" onclick={() => openSection(section.id)}>
            <section.icon size={18} strokeWidth={1.8} />
            <span>{section.label}</span>
            <ChevronRight size={16} class="chev" />
          </button>
        {/each}
      </div>
    {/if}
  {:else}
    <div class="settings-header">
      <SettingsIcon size={18} />
      <h1>设置</h1>
    </div>

    <nav class="settings-nav" aria-label="设置分区">
      {#each visibleSections as section (section.id)}
        <button
          class:active={navActive === section.id}
          onclick={() => scrollToSection(section.id)}
        >
          <section.icon size={14} />
          <span>{section.label}</span>
        </button>
      {/each}
    </nav>

    <div class="settings-sections">
      {#each visibleSections as section (section.id)}
        {@const Section = section.component}
        <Section />
      {/each}
    </div>
  {/if}
</div>

<style>
  .settings-view {
    max-width: 760px;
    padding: 24px 32px 48px;
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  .settings-view.mobile {
    max-width: none;
    padding: 0 0 24px;
    gap: 10px;
  }

  .settings-header {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .settings-header h1 {
    margin: 0;
    font-size: 18px;
    font-weight: 600;
  }

  /* Desktop: sticky section chips, also the fallback for narrow desktops. */
  .settings-nav {
    position: sticky;
    top: 0;
    z-index: 5;
    display: flex;
    flex-wrap: wrap;
    gap: 2px;
    padding: 6px 0;
    background: var(--color-base-100);
    border-bottom: 1px solid var(--panel-border);
  }
  .settings-nav button {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 5px 10px;
    border: 0;
    border-radius: 6px;
    background: none;
    color: color-mix(in oklch, var(--color-base-content) 65%, transparent);
    font-size: 12px;
    cursor: pointer;
  }
  .settings-nav button:hover {
    background: var(--color-base-200);
    color: var(--color-base-content);
  }
  .settings-nav button.active {
    background: color-mix(in oklch, var(--color-primary) 12%, transparent);
    color: var(--color-primary);
  }

  .settings-sections {
    display: flex;
    flex-direction: column;
    gap: 26px;
  }

  /* ── Phone: category list → one section at a time ── */
  .section-head {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 10px 8px 4px;
  }
  .section-head h1 {
    margin: 0;
    padding-left: 2px;
    font-size: 17px;
    font-weight: 600;
  }
  .back-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 36px;
    height: 36px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: inherit;
    cursor: pointer;
  }
  .back-btn:active {
    background: var(--color-base-300);
  }

  .category-list {
    display: flex;
    flex-direction: column;
    padding: 8px 8px 0;
  }
  .category {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    min-height: 52px;
    padding: 0 12px;
    border: 0;
    border-bottom: 1px solid var(--panel-border);
    background: none;
    color: var(--color-base-content);
    font-size: 14px;
    text-align: left;
    cursor: pointer;
  }
  .category:active {
    background: var(--color-base-200);
  }
  .category :global(.chev) {
    margin-left: auto;
    opacity: 0.4;
  }

  .mobile-sections {
    display: flex;
    flex-direction: column;
    gap: 18px;
    padding: 0 12px;
  }
</style>
