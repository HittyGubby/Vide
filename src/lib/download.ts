import { downloadTarget } from "$lib/transport";

function filename(title: string, id: string): string {
  const base = title.replace(/[\\/:*?"<>|]/g, "_").trim() || id.slice(0, 12);
  return base + ".mp3";
}

/**
 * Web bulk download. Browsers cannot return a directory from a save-file
 * dialog, so use the File System Access directory picker when available.
 * Fall back to normal browser downloads otherwise.
 */
export async function downloadManyWeb(songs: { id: string; title: string }[]): Promise<void> {
  const picker = (window as unknown as {
    showDirectoryPicker?: () => Promise<{
      getFileHandle: (name: string, options: { create: boolean }) => Promise<{
        createWritable: () => Promise<{ write: (data: Blob) => Promise<void>; close: () => Promise<void> }>;
      }>;
    }>;
  }).showDirectoryPicker;

  if (picker) {
    try {
      const directory = await picker();
      for (const song of songs) {
        const target = downloadTarget(song.id);
        if (!target) continue;
        const response = await fetch(target.url, { credentials: "same-origin" });
        if (!response.ok) continue;
        const handle = await directory.getFileHandle(filename(song.title, song.id), { create: true });
        const writable = await handle.createWritable();
        await writable.write(await response.blob());
        await writable.close();
      }
      return;
    } catch {
      /* fall through to normal downloads */
    }
  }

  for (const song of songs) {
    const target = downloadTarget(song.id);
    if (!target) continue;
    const link = document.createElement("a");
    link.href = target.url;
    link.download = filename(song.title, song.id);
    link.click();
  }
}
