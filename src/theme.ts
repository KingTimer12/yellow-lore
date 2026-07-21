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

// "The Amber Codex": a night scriptorium. The name Yellow Lore is taken
// literally — the accent is lamplight gold, set against warm ink (dark) and
// aged parchment (light). Hues sit in the warm 60–90° band, never cold blue.
export const DARK: ThemeTokens = {
  bg: "oklch(0.168 0.014 66)",
  panel: "oklch(0.212 0.016 68)",
  sidebar: "oklch(0.138 0.013 64)",
  fg: "oklch(0.935 0.018 82)",
  "fg-muted": "oklch(0.66 0.028 78)",
  border: "oklch(0.305 0.02 70)",
  hover: "oklch(0.258 0.018 70)",
  accent: "oklch(0.805 0.135 82)",
  "accent-fg": "oklch(0.20 0.035 70)",
  "accent-soft": "oklch(0.805 0.135 82 / 0.15)",
  success: "oklch(0.72 0.12 152)",
  "success-soft": "oklch(0.72 0.12 152 / 0.16)",
  warning: "oklch(0.74 0.145 52)",
  "warning-soft": "oklch(0.74 0.145 52 / 0.16)",
  danger: "oklch(0.64 0.185 28)",
};

export const LIGHT: ThemeTokens = {
  bg: "oklch(0.963 0.016 88)",
  panel: "oklch(0.992 0.008 88)",
  sidebar: "oklch(0.942 0.018 86)",
  fg: "oklch(0.255 0.022 62)",
  "fg-muted": "oklch(0.48 0.028 64)",
  border: "oklch(0.865 0.02 82)",
  hover: "oklch(0.922 0.02 84)",
  accent: "oklch(0.585 0.13 68)",
  "accent-fg": "oklch(0.99 0.012 86)",
  "accent-soft": "oklch(0.585 0.13 68 / 0.12)",
  success: "oklch(0.52 0.13 152)",
  "success-soft": "oklch(0.52 0.13 152 / 0.13)",
  warning: "oklch(0.58 0.14 55)",
  "warning-soft": "oklch(0.58 0.14 55 / 0.14)",
  danger: "oklch(0.55 0.19 28)",
};

export function themeVars(theme: "dark" | "light"): Record<string, string> {
  const t = theme === "dark" ? DARK : LIGHT;
  return Object.fromEntries(
    Object.entries(t).map(([k, v]) => [`--${k}`, v]),
  );
}

// Warm, codex-friendly avatar hues (amber, rust, wine, moss, bronze).
export const AVATAR_HUES = [
  "oklch(0.70 0.14 78)",
  "oklch(0.62 0.16 40)",
  "oklch(0.55 0.15 15)",
  "oklch(0.60 0.12 150)",
  "oklch(0.58 0.10 95)",
];

export function initials(name: string): string {
  const parts = name.split(" ").filter(Boolean);
  return ((parts[0]?.[0] ?? "") + (parts[1]?.[0] ?? "")).toUpperCase();
}

export type ProviderMeta = { id: string; label: string; hint: string };

// Ollama (local), OpenAI and vLLM (OpenAI-compatible server) are wired in the
// Rust backend. LLM and embedding pick a provider independently.
export const LLM_PROVIDERS: ProviderMeta[] = [
  { id: "ollama", label: "Ollama (local)", hint: "Modelos locais, sem API key" },
  { id: "openai", label: "OpenAI", hint: "GPT-4o, GPT-4.1, o-series" },
  { id: "vllm", label: "vLLM", hint: "Servidor OpenAI-compatível (self-hosted)" },
];

export const EMBED_PROVIDERS: ProviderMeta[] = [
  { id: "ollama", label: "Ollama (local)", hint: "nomic-embed-text, mxbai-embed-large" },
  { id: "openai", label: "OpenAI", hint: "text-embedding-3-small / large" },
  { id: "vllm", label: "vLLM", hint: "Modelos de embedding servidos via vLLM" },
];

export function providerLabel(id: string): string {
  return LLM_PROVIDERS.find((p) => p.id === id)?.label ?? id;
}
