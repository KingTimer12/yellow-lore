import { createSignal } from "solid-js";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauri } from "./api";

/// Tracks whether the OS window is maximized so the shell can drop its rounded
/// corners / shadow gutter when filling the screen.
export const [maximized, setMaximized] = createSignal(false);

function win() {
  return getCurrentWindow();
}

/// Subscribe to resize/maximize changes. Call once on mount (Tauri only).
export async function initWindow() {
  if (!isTauri) return;
  const w = win();
  try {
    setMaximized(await w.isMaximized());
    await w.onResized(async () => setMaximized(await w.isMaximized()));
  } catch (e) {
    console.error("initWindow", e);
  }
}

/// Custom titlebar window controls (no-ops outside Tauri).
export const winCtl = {
  minimize: () => { if (isTauri) void win().minimize(); },
  toggleMaximize: () => { if (isTauri) void win().toggleMaximize(); },
  close: () => { if (isTauri) void win().close(); },
};
