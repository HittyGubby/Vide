import { invoke } from "@tauri-apps/api/core";

class PlayerStore {
  /** Currently playing song ID, or null if stopped. */
  mediaId = $state<string | null>(null);
  title = $state("未在播放");
  artist = $state("选择一首歌曲开始播放");
  album = $state("");
  cover = $state<string | null>(null);
  elapsed = $state(0);
  duration = $state(0);
  isPlaying = $state(false);

  /** Poll the backend for current playback status. */
  async poll() {
    try {
      const status: {
        state: string;
        media_id?: string;
        elapsed_secs?: number;
        duration_secs?: number;
      } = await invoke("get_player_status");

      this.isPlaying = status.state === "Playing";
      this.elapsed = status.elapsed_secs ?? 0;
      this.duration = status.duration_secs ?? 0;

      const newId = status.media_id ?? null;
      if (newId && newId !== this.mediaId) {
        this.mediaId = newId;
        // Fetch song details
        try {
          const song: {
            title: string;
            artist: string;
            album: string;
            paths: string[];
            metadata?: {
              title?: string;
              artist?: string;
              album?: string;
              display_title?: string;
              artist_override?: string;
            };
          } = await invoke("get_song", { id: newId });
          if (song) {
            this.title = song.metadata?.display_title ?? song.title;
            this.artist = song.metadata?.artist_override ?? song.artist;
            this.album = song.metadata?.album ?? song.album ?? "";
            // A local file can double as the cover placeholder (monogram is
            // shown when no artwork/cover is available).
            this.cover = song.paths[0] ?? null;
          }
        } catch {
          this.title = "Unknown";
          this.artist = "Unknown";
        }
      } else if (!newId) {
        this.mediaId = null;
        if (!this.isPlaying) {
          this.title = "未在播放";
          this.artist = "选择一首歌曲开始播放";
        }
      }
    } catch {
      /* backend not ready yet */
    }
  }
}

export const player = new PlayerStore();
