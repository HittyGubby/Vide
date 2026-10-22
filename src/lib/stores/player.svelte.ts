import {
  getPlaybackTarget,
  invoke,
  invokeLocal,
  isRemoteMode,
  remoteMediaUrl,
  subscribePlayerStatus,
} from "$lib/transport";
import { isTauri } from "$lib/isTauri";

function isRemoteClient(): boolean {
  return isRemoteMode() && getPlaybackTarget() === "client";
}

function isBrowserClient(): boolean {
  return !isTauri && getPlaybackTarget() === "client";
}

let browserAudio: HTMLAudioElement | null = null;

async function statusInvoke<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  return isRemoteClient() ? invokeLocal<T>(command, args) : invoke<T>(command, args);
}

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
  speed = $state(1.0);
  volume = $state(80);
  muted = $state(false);

  /** Set playback volume (0..=100) and persist to the backend (mpv). */
  async setVolume(v: number) {
    this.volume = Math.max(0, Math.min(100, Math.round(v)));
    if (this.volume > 0) this.muted = false;
    if (isBrowserClient()) {
      this.browserAudioElement().volume = this.volume / 100;
      return;
    }
    try {
      await statusInvoke("set_volume", { volume: this.volume });
    } catch (e) {
      console.error("set_volume error:", e);
    }
  }

  /** Mute / unmute playback. */
  async setMuted(m: boolean) {
    this.muted = m;
    if (isBrowserClient()) {
      this.browserAudioElement().muted = m;
      return;
    }
    try {
      await statusInvoke("set_mute", { muted: m });
    } catch (e) {
      console.error("set_mute error:", e);
    }
  }

  /** Set playback speed (rate) — persists to the backend too. */
  async setSpeed(rx: number) {
    this.speed = rx;
    if (isBrowserClient()) {
      this.browserAudioElement().playbackRate = rx;
      return;
    }
    try {
      await statusInvoke("set_speed", { speed: rx });
    } catch (e) {
      console.error("set_speed error:", e);
    }
  }

  // ── Playback queue / order ─────────────────────────
  // The current playlist and the index of the actively playing song.
  queue = $state<{ id: string; title: string; artist: string; album: string }[]>([]);
  currentIndex = $state(-1);

  // Play order mode — mirrors the Rust PlayMode and is pushed to the backend.
  playMode = $state<{ single: boolean; reverse: boolean; shuffle: boolean; repeat: boolean }>({
    single: false,   // false = play list, true = repeat one
    reverse: false,  // false = forward, true = reverse
    shuffle: false,  // false = in order, true = random
    repeat: true,    // false = stop after list, true = loop
  });

  /** Push the current play mode to the Rust backend (which owns sequencing). */
  syncPlayMode() {
    if (isBrowserClient()) return;
    try {
      void statusInvoke("set_play_mode", { mode: { ...this.playMode } }).catch((e) => {
        console.error("set_play_mode error:", e);
      });
    } catch (e) {
      console.error("set_play_mode error:", e);
    }
  }
  // Tracks already played in the current shuffle round (reset when exhausted).
  _shufflePlayed = new Set<string>();
  resetShuffle() { this._shufflePlayed = new Set<string>(); }

  // Monotonic request id used by the player bar's "locate current song" action.
  // Using a plain counter (instead of a URL query) means repeated right-clicks
  // on the same song always fire, because the id changes every time.
  locateRequestId = $state(0);
  locateSongId = $state<string | null>(null);
  requestLocate(songId: string) {
    this.locateSongId = songId;
    this.locateRequestId++;
  }

  _saveTick = 0;
  // If set, the next play should seek to this position (startup restore).
  _pendingSeek = 0;
  private statusTimer: ReturnType<typeof setInterval> | undefined;
  private statusStream: (() => void) | null = null;
  private updatesStarted = false;

  private browserAudioElement(): HTMLAudioElement {
    if (!browserAudio) {
      const audio = new Audio();
      audio.preload = "auto";
      browserAudio = audio;
      audio.addEventListener("timeupdate", () => {
        this.elapsed = audio.currentTime || 0;
      });
      audio.addEventListener("durationchange", () => {
        this.duration = Number.isFinite(audio.duration) ? audio.duration : 0;
      });
      audio.addEventListener("play", () => {
        this.isPlaying = true;
      });
      audio.addEventListener("pause", () => {
        this.isPlaying = false;
      });
      audio.addEventListener("ended", () => {
        this.isPlaying = false;
        void this.playNext(true);
      });
    }
    return browserAudio;
  }

  /** Set the active queue (the list the user is playing from) and register it
   * on the Rust backend, which owns all sequencing. */
  async setQueue(items: { id: string; title: string; artist: string; album: string }[]) {
    this.queue = items;
    this.currentIndex = items.findIndex((i) => i.id === this.mediaId);
    if (isRemoteClient() || isBrowserClient()) return;
    try {
      await invoke("set_play_queue", {
        ids: items.map((i) => i.id),
        startIndex: this.currentIndex,
      });
    } catch (e) {
      console.error("set_play_queue error:", e);
    }
  }

  /** Register the queue and start playing `id`, all owned by the backend. */
  async playFromList(items: { id: string; title: string; artist: string; album: string }[], id: string) {
    this.queue = items;
    this.currentIndex = items.findIndex((i) => i.id === id);
    if (isRemoteClient() || isBrowserClient()) {
      await this.playId(id);
      return;
    }
    const idx = this.currentIndex;
    try {
      // Tell the backend the queue and which index to start playing.
      await invoke("set_play_queue", {
        ids: items.map((i) => i.id),
        startIndex: idx,
      });
      this.isPlaying = true;
    } catch (e) {
      console.error("set_play_queue error:", e);
    }
  }

  /** Toggle play/pause without discarding the current song.
   * Unlike the old stop_playback path, this keeps mediaId/title/artist so
   * the player bar and detail page are not reset when pausing. */
  async togglePause() {
    try {
      if (isBrowserClient()) {
        const audio = this.browserAudioElement();
        if (audio.paused) await audio.play();
        else audio.pause();
        return;
      }
      const status = await statusInvoke<{
        state: string;
        media_id?: string;
        elapsed_secs?: number;
        duration_secs?: number;
      }>("toggle_pause");
      // Nothing is loaded (restored on startup) or the track already ran to
      // its end: a plain toggle is a no-op, so start the remembered song.
      if (this.mediaId && (status.state === "Stopped" || status.state === "Ended")) {
        if (status.state === "Ended") {
          // The track finished — replay it rather than doing nothing.
          await this.playId(this.mediaId, 0);
          return;
        }
        // Prefer _pendingSeek; fall back to the position the UI is showing.
        const saved = this._pendingSeek > 0 ? this._pendingSeek : this.elapsed;
        // Resuming inside the last moment of a track ends instantly and used
        // to leave the player wedged in its "ended" state, so start over.
        const resume = this.duration > 0 && saved >= this.duration - 2 ? 0 : saved;
        await this.playId(this.mediaId, resume);
        return;
      }
      this.isPlaying = status.state === "Playing";
      this.elapsed = status.elapsed_secs ?? this.elapsed;
      this.duration = status.duration_secs ?? this.duration;
      if (status.media_id) this.mediaId = status.media_id;
    } catch (e) {
      console.error("toggle_pause error:", e);
    }
  }

  /** Legitimately stop (deselect) the current song. */
  async stop() {
    try {
      if (isBrowserClient()) {
        const audio = this.browserAudioElement();
        audio.pause();
        audio.removeAttribute("src");
        audio.load();
        this.isPlaying = false;
        this.mediaId = null;
        this.cover = null;
        this.title = "未在播放";
        this.artist = "选择一首歌曲开始播放";
        this.duration = 0;
        this.elapsed = 0;
        return;
      }
      await statusInvoke("stop_playback");
      this.isPlaying = false;
      this.mediaId = null;
      this.cover = null;
      this.title = "未在播放";
      this.artist = "选择一首歌曲开始播放";
      this.duration = 0;
      this.elapsed = 0;
    } catch (e) {
      console.error("stop error:", e);
    }
  }

  async seekTo(target: number) {
    if (this.duration <= 0) return;
    const position = Math.max(0, Math.min(this.duration, target));
    try {
      if (isBrowserClient()) {
        this.browserAudioElement().currentTime = position;
      } else {
        await statusInvoke("seek", { positionSecs: position });
      }
      this.elapsed = position;
    } catch (e) {
      console.error("seek error:", e);
    }
  }

  /** Seek by a relative amount (seconds) — used by the seek shortcuts. */
  async seekBy(delta: number) {
    if (!this.mediaId || this.duration <= 0) return;
    const target = Math.max(0, Math.min(this.duration, this.elapsed + delta));
    try {
      if (isBrowserClient()) {
        this.browserAudioElement().currentTime = target;
        this.elapsed = target;
        return;
      }
      await statusInvoke("seek", { positionSecs: target });
      this.elapsed = target;
    } catch (e) {
      console.error("seek error:", e);
    }
  }

  /** Play the next song — sequencing is decided by the Rust backend. */
  async playNext(auto = false): Promise<boolean> {
    try {
      if (isRemoteClient() || isBrowserClient()) {
        if (this.queue.length === 0) return false;
        const current = this.currentIndex >= 0 ? this.currentIndex : 0;
        if (this.playMode.single) {
          await this.playId(this.queue[current].id);
          return true;
        }
        let next = current;
        if (this.playMode.shuffle && this.queue.length > 1) {
          do {
            next = Math.floor(Math.random() * this.queue.length);
          } while (next === current);
        } else {
          next = current + (this.playMode.reverse ? -1 : 1);
          if (next < 0 || next >= this.queue.length) {
            if (!this.playMode.repeat) {
              await this.stop();
              return false;
            }
            next = this.playMode.reverse ? this.queue.length - 1 : 0;
          }
        }
        await this.playId(this.queue[next].id);
        return true;
      }
      const status = await invoke<{
        state: string;
        media_id?: string;
        elapsed_secs?: number;
        duration_secs?: number;
      }>("play_next");
      await this._syncStatus(status);
      return true;
    } catch (e) {
      console.error("play_next error:", e);
      return false;
    }
  }

  /** Play the previous song — sequencing handled by the backend. */
  async playPrev(): Promise<boolean> {
    try {
      if (isRemoteClient() || isBrowserClient()) {
        if (this.queue.length === 0) return false;
        const prev = (this.currentIndex <= 0 ? this.queue.length : this.currentIndex) - 1;
        await this.playId(this.queue[prev].id);
        return true;
      }
      const status = await invoke<{
        state: string;
        media_id?: string;
        elapsed_secs?: number;
        duration_secs?: number;
      }>("play_prev");
      await this._syncStatus(status);
      return true;
    } catch (e) {
      console.error("play_prev error:", e);
      return false;
    }
  }

  /** Play a specific song id (tells the backend to play it immediately).
   * `resumeAt` (seconds) is applied by the backend once mpv is ready. */
  async playId(id: string, resumeAt = 0) {
    const position = resumeAt > 0 ? resumeAt : this._pendingSeek;
    this._pendingSeek = 0;
    try {
      if (isBrowserClient()) {
        const media = remoteMediaUrl(id);
        if (!media) throw new Error("browser media URL unavailable");
        const audio = this.browserAudioElement();
        this.mediaId = id;
        const queueIndex = this.queue.findIndex((item) => item.id === id);
        if (queueIndex >= 0) this.currentIndex = queueIndex;
        this.elapsed = 0;
        this.duration = 0;
        audio.src = media.url;
        audio.volume = this.volume / 100;
        audio.muted = this.muted;
        audio.playbackRate = this.speed;
        await audio.play();
        if (position > 0) {
          audio.currentTime = position;
          this.elapsed = position;
        }
        try {
          const song = await invoke<{
            id: string;
            title: string;
            artist: string;
            album: string;
            metadata?: {
              display_title?: string;
              artist_override?: string;
              album_override?: string;
              cover_url?: string;
            };
          }>("get_song", { id });
          this.title = song.metadata?.display_title ?? song.title;
          this.artist = song.metadata?.artist_override ?? song.artist;
          this.album = song.metadata?.album_override ?? song.album ?? "";
          this.cover = song.metadata?.cover_url ?? null;
        } catch {
          /* keep queue metadata */
        }
        return;
      }
      if (isRemoteClient()) {
        const media = remoteMediaUrl(id);
        if (!media) throw new Error("remote media URL unavailable");
        const status = await invokeLocal<{
          state: string;
          media_id?: string;
          elapsed_secs?: number;
          duration_secs?: number;
        }>("play_remote_media", {
          id,
          url: media.url,
          headers: media.headers,
          positionSecs: position > 0 ? position : null,
        });
        await this._syncStatus(status);
        return;
      }
      const status = await invoke<{
        state: string;
        media_id?: string;
        elapsed_secs?: number;
        duration_secs?: number;
      }>("play_song", {
        id,
        positionSecs: position > 0 ? position : null,
      });
      await this._syncStatus(status);
    } catch (e) {
      console.error("play error:", e);
    }
  }

  /** Apply a backend status snapshot to this store. */
  async _syncStatus(status: { state: string; media_id?: string; elapsed_secs?: number; duration_secs?: number }) {
    this.isPlaying = status.state === "Playing";
    this.elapsed = status.elapsed_secs ?? this.elapsed;
    this.duration = status.duration_secs ?? this.duration;
    const newId = status.media_id ?? null;
    if (newId && newId !== this.mediaId) {
      this.mediaId = newId;
      // Fetch song details so the player bar updates when the track changes.
      try {
        const song = await invoke<{
          title: string;
          artist: string;
          album: string;
          metadata?: {
            display_title?: string;
            artist_override?: string;
            album_override?: string;
            album?: string;
            cover_url?: string;
          };
        }>("get_song", { id: newId });
        if (song) {
          this.title = song.metadata?.display_title ?? song.title;
          this.artist = song.metadata?.artist_override ?? song.artist;
          this.album = song.metadata?.album_override ?? song.metadata?.album ?? song.album ?? "";
          this.cover = song.metadata?.cover_url ?? null;
        }
      } catch {
        /* not fatal */
      }
    } else if (newId) {
      this.mediaId = newId;
    }
    // Refresh the queued index for the media id.
    this.currentIndex = this.queue.findIndex((i) => i.id === this.mediaId);
  }

  /** Save the current song id + position (throttled) so we can resume. */
  async saveProgress() {
    if (!this.mediaId) return;
    try {
      await invoke("save_last_played", {
        songId: this.mediaId,
        positionSecs: Math.round(this.elapsed * 10) / 10,
      });
    } catch {
      /* non-fatal */
    }
  }

  /** Load the last-played snapshot, if any. */
  async getLastPlayed(): Promise<{ songId: string; position: number } | null> {
    try {
      const r = await invoke<[string, number] | { song_id: string; position: number } | null>(
        "get_last_played",
      );
      if (!r) return null;
      // Backend may return a tuple or an object.
      if (Array.isArray(r)) {
        return { songId: r[0], position: r[1] ?? 0 };
      }
      return { songId: r.song_id, position: r.position ?? 0 };
    } catch {
      return null;
    }
  }

  stopStatusUpdates() {
    if (this.statusTimer) {
      clearInterval(this.statusTimer);
      this.statusTimer = undefined;
    }
    this.statusStream?.();
    this.statusStream = null;
    this.updatesStarted = false;
  }

  restartStatusUpdates() {
    this.stopStatusUpdates();
    this.startStatusUpdates();
  }

  startStatusUpdates() {
    if (this.updatesStarted || isBrowserClient()) return;
    this.updatesStarted = true;
    this.syncPlayMode();
    void this.poll();
    if (isRemoteClient() || (isTauri && !isRemoteMode())) {
      this.statusTimer = setInterval(() => void this.poll(), 1000);
      return;
    }
    this.statusStream = subscribePlayerStatus((status) => {
      void this.applyStatus(status);
    });
    if (!this.statusStream) {
      this.statusTimer = setInterval(() => void this.poll(), 1000);
    }
  }

  /** Poll the backend for current playback status. */
  async poll() {
    if (isBrowserClient()) return;
    try {
      const status: {
        state: string;
        media_id?: string;
        elapsed_secs?: number;
        duration_secs?: number;
      } = isRemoteClient()
        ? await invokeLocal("get_player_status")
        : await invoke("get_player_status");
      await this.applyStatus(status);
    } catch {
      /* backend not ready yet */
    }
  }

  private async applyStatus(status: {
    state: string;
    media_id?: string;
    elapsed_secs?: number;
    duration_secs?: number;
  }) {
      this.isPlaying = status.state === "Playing";
      this.elapsed = status.elapsed_secs ?? 0;
      // mpv reports no duration until the new file is fully opened. Keeping
      // the previous value avoids the bar disappearing and reappearing (and
      // the resulting jump) on every track change.
      this.duration = status.duration_secs ?? this.duration;

      // A remote client uses the local mpv directly, so its local queue is
      // intentionally empty. Let the frontend queue handle the natural end.
      if (status.state === "Ended" && isRemoteClient() && this.queue.length > 0) {
        void this.playNext(true);
      }

      const newId = status.media_id ?? null;
      // Remember progress while playing (save ~every 5 seconds of position change).
      this._saveTick = (this._saveTick ?? 0) + 1;
      // Save the last-played song + position every ~2s while playing so a
      // restart can restore precisely where the user left off.
      if (this.isPlaying && this._saveTick % 2 === 0) {
        void this.saveProgress();
      }

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
              album_override?: string;
              display_title?: string;
              artist_override?: string;
              cover_url?: string;
            };
          } = await invoke("get_song", { id: newId });
          if (song) {
            this.title = song.metadata?.display_title ?? song.title;
            this.artist = song.metadata?.artist_override ?? song.artist;
            this.album = song.metadata?.album_override ?? song.metadata?.album ?? song.album ?? "";
            // Remote cover URLs come from the source plugin; local files need
            // embedded-art extraction before they can use a local path here.
            this.cover = song.metadata?.cover_url ?? null;
          }
        } catch {
          this.title = "Unknown";
          this.artist = "Unknown";
        }
      } else if (!newId) {
        // Only reset to the idle "nothing selected" state when the backend
        // reports no media loaded (i.e. a real stop, not a pause).
        this.mediaId = null;
        this.cover = null;
        this.title = "未在播放";
        this.artist = "选择一首歌曲开始播放";
        this.duration = 0;
        this.elapsed = 0;
      }
  }
}

export const player = new PlayerStore();
