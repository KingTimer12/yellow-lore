import { createSignal } from "solid-js";
import { isTauri } from "./api";

// Auto-update state, driven by the Tauri updater plugin. The signed `latest.json`
// published by the release workflow is the source of truth; the plugin verifies
// it against the pubkey baked into tauri.conf.json before installing anything.

export type UpdateInfo = { version: string; notes: string };

export const [updateInfo, setUpdateInfo] = createSignal<UpdateInfo | null>(null);
export const [updateBusy, setUpdateBusy] = createSignal(false);
export const [updateProgress, setUpdateProgress] = createSignal(0); // 0..1
export const [updateError, setUpdateError] = createSignal<string | null>(null);
export const [updateDismissed, setUpdateDismissed] = createSignal(false);

// The resolved Update handle is kept between check and install.
let pending: import("@tauri-apps/plugin-updater").Update | null = null;

/// Ask the release endpoint whether a newer signed build exists. Silent on
/// failure (offline, no release yet) — updating is best-effort, never blocking.
export async function checkForUpdates(): Promise<void> {
  if (!isTauri) return;
  try {
    const { check } = await import("@tauri-apps/plugin-updater");
    const update = await check();
    if (update) {
      pending = update;
      setUpdateInfo({ version: update.version, notes: update.body ?? "" });
    }
  } catch (e) {
    console.error("update check falhou", e);
  }
}

/// Download + install the pending update, streaming progress, then relaunch into
/// the new version. On error the banner surfaces the message and stays actionable.
export async function installUpdate(): Promise<void> {
  if (!isTauri || !pending || updateBusy()) return;
  setUpdateBusy(true);
  setUpdateError(null);
  setUpdateProgress(0);
  try {
    let downloaded = 0;
    let total = 0;
    await pending.downloadAndInstall((event) => {
      switch (event.event) {
        case "Started":
          total = event.data.contentLength ?? 0;
          break;
        case "Progress":
          downloaded += event.data.chunkLength ?? 0;
          if (total > 0) setUpdateProgress(downloaded / total);
          break;
        case "Finished":
          setUpdateProgress(1);
          break;
      }
    });
    const { relaunch } = await import("@tauri-apps/plugin-process");
    await relaunch();
  } catch (e) {
    console.error("update install falhou", e);
    setUpdateError(String(e));
    setUpdateBusy(false);
  }
}

export function dismissUpdate(): void {
  setUpdateDismissed(true);
}
