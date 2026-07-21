import { getVersion } from "@tauri-apps/api/app";
import { createSignal } from "solid-js";
import { isTauri } from "./api";
import tauriConf from "../src-tauri/tauri.conf.json";

// Runtime app version. Seeded from tauri.conf for the browser preview, then
// overridden by the real value from Tauri's app API once running in the app.
export const [appVersion, setAppVersion] = createSignal<string>(tauriConf.version);

export async function loadAppVersion() {
  if (!isTauri) return;
  try {
    setAppVersion(await getVersion());
  } catch (e) {
    console.error("getVersion", e);
  }
}
