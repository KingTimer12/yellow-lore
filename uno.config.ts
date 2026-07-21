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
      // UI chrome stays in a clean humanist sans.
      sans: "-apple-system, 'Inter', 'Helvetica Neue', Helvetica, Arial, sans-serif",
      // Display: engraved caps for the brand / big titles (Cinzel → serif).
      display: "'Cinzel', 'Cormorant Garamond', 'Iowan Old Style', Georgia, serif",
      // Serif: characterful headings (Cormorant Garamond).
      serif: "'Cormorant Garamond', 'Iowan Old Style', Georgia, 'Times New Roman', serif",
      // Reading: the prose face for assistant answers (Crimson Pro).
      reading: "'Crimson Pro', 'Iowan Old Style', Georgia, serif",
      mono: "ui-monospace, 'SF Mono', 'JetBrains Mono', Menlo, monospace",
    },
  },
});
