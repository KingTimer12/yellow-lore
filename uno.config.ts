import { defineConfig, presetWind3, presetIcons } from "unocss";

// Theme tokens are exposed as CSS variables (see src/theme.ts). Mapping them
// here lets us use utilities like `bg-panel`, `text-fg-muted`, `border-border`
// while the actual values swap with the dark/light theme at runtime.
export default defineConfig({
  presets: [
    presetWind3(),
    presetIcons({ scale: 1.1, warn: true }),
  ],
  theme: {
    colors: {
      bg: "var(--bg)",
      fg: "var(--fg)",
      "fg-muted": "var(--fg-muted)",
      panel: "var(--panel)",
      sidebar: "var(--sidebar)",
      border: "var(--border)",
      hover: "var(--hover)",
      accent: "var(--accent)",
      "accent-fg": "var(--accent-fg)",
      "accent-soft": "var(--accent-soft)",
      success: "var(--success)",
      "success-soft": "var(--success-soft)",
      warning: "var(--warning)",
      "warning-soft": "var(--warning-soft)",
      danger: "var(--danger)",
    },
    fontFamily: {
      sans: "-apple-system, 'Helvetica Neue', Helvetica, Arial, sans-serif",
      mono: "ui-monospace, 'SF Mono', Menlo, monospace",
    },
  },
});
