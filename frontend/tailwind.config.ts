import type { Config } from "tailwindcss";

export default {
  content: [
    "./src/pages/**/*.{js,ts,jsx,tsx,mdx}",
    "./src/components/**/*.{js,ts,jsx,tsx,mdx}",
    "./src/app/**/*.{js,ts,jsx,tsx,mdx}",
  ],
  theme: {
    extend: {
      colors: {
        background: "var(--background)",
        foreground: "var(--foreground)",
        primary: "hsl(var(--primary))",
        secondary: "hsl(var(--secondary))",
        accent: "hsl(var(--accent))",
        destructive: "hsl(var(--destructive))",
        briefli: {
          paper: "var(--briefli-paper)",
          surface: "var(--briefli-surface)",
          sidebar: "var(--briefli-sidebar)",
          ink: "var(--briefli-ink)",
          muted: "var(--briefli-muted)",
          line: "var(--briefli-line)",
          capture: "var(--briefli-capture)",
          "capture-dark": "var(--briefli-capture-dark)",
          confirmed: "var(--briefli-confirmed)",
          caution: "var(--briefli-caution)",
        },
      },
      fontFamily: {
        sans: ["var(--font-plex-sans)", "sans-serif"],
        brand: ["var(--font-lora)", "serif"],
      },
      fontSize: {
        'display': ['32px', { lineHeight: '1.2', fontWeight: '700' }],
        'h1': ['24px', { lineHeight: '1.3', fontWeight: '600' }],
        'h2': ['18px', { lineHeight: '1.4', fontWeight: '500' }],
        'body': ['16px', { lineHeight: '1.6', fontWeight: '400' }],
        'small': ['14px', { lineHeight: '1.5', fontWeight: '400' }],
        'caption': ['12px', { lineHeight: '1.4', fontWeight: '400' }],
      },
    },
  },
  plugins: [
    require('@tailwindcss/typography'),
  ],
} satisfies Config;
