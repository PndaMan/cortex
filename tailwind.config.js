/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{svelte,js,ts}"],
  theme: {
    // Cortex uses CSS custom properties (cortex.css) as the source of truth so
    // it can re-skin live with the Omarchy palette. Tailwind maps semantic
    // tokens onto those variables rather than hardcoding hex.
    extend: {
      colors: {
        bg: "var(--bg)",
        "bg-sunken": "var(--bg-sunken)",
        surface: "var(--surface)",
        "surface-2": "var(--surface-2)",
        "surface-3": "var(--surface-3)",
        border: "var(--border)",
        "border-strong": "var(--border-strong)",
        fg: "var(--fg)",
        "fg-bright": "var(--fg-bright)",
        "fg-muted": "var(--fg-muted)",
        "fg-faint": "var(--fg-faint)",
        accent: "var(--accent)",
        "accent-dim": "var(--accent-dim)",
        "accent-fg": "var(--accent-fg)",
        ok: "var(--ok)",
        warn: "var(--warn)",
        err: "var(--err)",
        info: "var(--info)",
      },
      fontFamily: {
        mono: "var(--font-mono)",
        read: "var(--font-read)",
        sans: "var(--font-sans)",
      },
      borderRadius: {
        1: "var(--rad-1)",
        2: "var(--rad-2)",
        3: "var(--rad-3)",
        4: "var(--rad-4)",
      },
    },
  },
  plugins: [],
};
