// Design tokens. Values are OKLCH, applied as CSS custom properties on the app
// root so UnoCSS utilities (bg-panel, text-fg-muted, ...) resolve to the active
// theme. Keys map 1:1 to the `--*` variable names.
export type ThemeTokens = {
  bg: string;
  panel: string;
  sidebar: string;
  fg: string;
  "fg-muted": string;
  border: string;
  hover: string;
  accent: string;
  "accent-fg": string;
  "accent-soft": string;
  success: string;
  "success-soft": string;
  warning: string;
  "warning-soft": string;
  danger: string;
};

export const DARK: ThemeTokens = {
  bg: "oklch(0.17 0.01 260)",
  panel: "oklch(0.205 0.012 260)",
  sidebar: "oklch(0.14 0.01 260)",
  fg: "oklch(0.95 0.004 260)",
  "fg-muted": "oklch(0.64 0.012 260)",
  border: "oklch(0.30 0.012 260)",
  hover: "oklch(0.26 0.012 260)",
  accent: "oklch(0.68 0.17 255)",
  "accent-fg": "oklch(0.99 0.005 255)",
  "accent-soft": "oklch(0.68 0.17 255 / 0.16)",
  success: "oklch(0.68 0.15 145)",
  "success-soft": "oklch(0.68 0.15 145 / 0.16)",
  warning: "oklch(0.75 0.14 80)",
  "warning-soft": "oklch(0.75 0.14 80 / 0.16)",
  danger: "oklch(0.63 0.18 25)",
};

export const LIGHT: ThemeTokens = {
  bg: "oklch(0.985 0.003 260)",
  panel: "oklch(1 0 0)",
  sidebar: "oklch(0.965 0.005 260)",
  fg: "oklch(0.24 0.01 260)",
  "fg-muted": "oklch(0.50 0.012 260)",
  border: "oklch(0.885 0.008 260)",
  hover: "oklch(0.94 0.006 260)",
  accent: "oklch(0.56 0.16 255)",
  "accent-fg": "oklch(0.99 0.005 255)",
  "accent-soft": "oklch(0.56 0.16 255 / 0.10)",
  success: "oklch(0.55 0.14 145)",
  "success-soft": "oklch(0.55 0.14 145 / 0.12)",
  warning: "oklch(0.60 0.14 70)",
  "warning-soft": "oklch(0.60 0.14 70 / 0.14)",
  danger: "oklch(0.55 0.18 25)",
};

export function themeVars(theme: "dark" | "light"): Record<string, string> {
  const t = theme === "dark" ? DARK : LIGHT;
  return Object.fromEntries(
    Object.entries(t).map(([k, v]) => [`--${k}`, v]),
  );
}

export const AVATAR_HUES = [
  "oklch(0.62 0.15 255)",
  "oklch(0.62 0.15 20)",
  "oklch(0.62 0.15 320)",
  "oklch(0.62 0.15 145)",
];

export function initials(name: string): string {
  const parts = name.split(" ").filter(Boolean);
  return ((parts[0]?.[0] ?? "") + (parts[1]?.[0] ?? "")).toUpperCase();
}

export type ProviderMeta = { id: string; label: string; hint: string };

// Only Ollama (local) and OpenAI are wired in the Rust backend. LLM and
// embedding pick a provider independently.
export const LLM_PROVIDERS: ProviderMeta[] = [
  { id: "ollama", label: "Ollama (local)", hint: "Modelos locais, sem API key" },
  { id: "openai", label: "OpenAI", hint: "GPT-4o, GPT-4.1, o-series" },
];

export const EMBED_PROVIDERS: ProviderMeta[] = [
  { id: "ollama", label: "Ollama (local)", hint: "nomic-embed-text, mxbai-embed-large" },
  { id: "openai", label: "OpenAI", hint: "text-embedding-3-small / large" },
];

export function providerLabel(id: string): string {
  return LLM_PROVIDERS.find((p) => p.id === id)?.label ?? id;
}
