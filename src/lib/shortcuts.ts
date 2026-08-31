import { getCurrentWindow } from "@tauri-apps/api/window";
import { player } from "$lib/stores/player.svelte";
import { settingsStore } from "$lib/stores/settings.svelte";
export {
  acceleratorFromEvent,
  eventMatchesAccelerator,
  formatAccelerator,
} from "$lib/shortcutKeys";
import type { ShortcutAction, ShortcutBinding } from "$lib/types";

/** Actions offered in the settings table, in display order. */
export const SHORTCUT_ACTIONS: { id: ShortcutAction; label: string }[] = [
  { id: "toggle_window", label: "显示/隐藏窗口" },
  { id: "toggle_lyrics", label: "显示/隐藏悬浮歌词" },
  { id: "quit", label: "退出应用" },
  { id: "toggle_play", label: "暂停/播放" },
  { id: "prev_track", label: "上一首" },
  { id: "next_track", label: "下一首" },
  { id: "volume_down", label: "减小音量" },
  { id: "volume_up", label: "增大音量" },
  { id: "seek_forward", label: "快进" },
  { id: "seek_back", label: "快退" },
];

export function defaultShortcuts(): Record<ShortcutAction, ShortcutBinding> {
  // Global defaults are the in-app ones with Shift added.
  const both = (inApp: string, global: string): ShortcutBinding => ({ in_app: inApp, global });
  return {
    toggle_window: both("", ""),
    toggle_lyrics: both("", ""),
    quit: both("Alt+F4", "Shift+Alt+F4"),
    toggle_play: both("Space", "Shift+Space"),
    prev_track: both("PageUp", "Shift+PageUp"),
    next_track: both("PageDown", "Shift+PageDown"),
    volume_down: both("ArrowDown", "Shift+ArrowDown"),
    volume_up: both("ArrowUp", "Shift+ArrowUp"),
    seek_forward: both("ArrowRight", "Shift+ArrowRight"),
    seek_back: both("ArrowLeft", "Shift+ArrowLeft"),
  };
}

/** Run the action bound to a shortcut. */
export async function runShortcutAction(action: ShortcutAction): Promise<void> {
  const settings = settingsStore.settings;
  switch (action) {
    case "toggle_window": {
      const win = getCurrentWindow();
      if (await win.isVisible()) {
        await win.hide();
      } else {
        await win.show();
        await win.setFocus();
      }
      break;
    }
    case "toggle_lyrics":
      // Visibility lives in the settings; the root layout mirrors it onto the
      // desktop lyrics window.
      await settingsStore.update(
        "desktop_lyrics_enabled",
        !settingsStore.settings.desktop_lyrics_enabled,
      );
      break;
    case "quit":
      await getCurrentWindow().close();
      break;
    case "toggle_play":
      await player.togglePause();
      break;
    case "prev_track":
      await player.playPrev();
      break;
    case "next_track":
      await player.playNext();
      break;
    case "volume_down":
      await player.setVolume(player.volume - settings.volume_step);
      break;
    case "volume_up":
      await player.setVolume(player.volume + settings.volume_step);
      break;
    case "seek_back":
      await player.seekBy(-settings.seek_step_secs);
      break;
    case "seek_forward":
      await player.seekBy(settings.seek_step_secs);
      break;
  }
}