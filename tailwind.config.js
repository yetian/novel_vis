/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{vue,js,ts,jsx,tsx}"],
  theme: {
    extend: {
      colors: {
        ink: {
          900: "#0c1119",
          800: "#121925",
          700: "#1a2231",
          600: "#242e3f",
          500: "#334054",
        },
        slate: {
          50: "#f8fafc",
          100: "#f1f5f9",
          200: "#e5eaf1",
          300: "#c7d0dd",
          400: "#a3afc2",
          500: "#8794a8",
          600: "#6b788c",
          700: "#525e70",
          800: "#394352",
          900: "#222a36",
          950: "#141a23",
        },
        jade: {
          300: "#63e6be",
          400: "#38d9a9",
          500: "#20c997",
          600: "#12b886",
        },
        gold: {
          300: "#ffd43b",
          400: "#f0b429",
          500: "#e6a700",
        },
        cinnabar: {
          400: "#ff6b6b",
          500: "#f03e3e",
        },
      },
      fontFamily: {
        sans: ["Inter", "Noto Sans SC", "PingFang SC", "Microsoft YaHei", "sans-serif"],
        serif: [
          "Noto Serif SC",
          "Noto Serif CJK SC",
          "Source Han Serif SC",
          "Songti SC",
          "SimSun",
          "serif",
        ],
        mono: ["JetBrains Mono", "Menlo", "monospace"],
      },
      boxShadow: {
        glow: "0 0 24px -6px rgba(32, 201, 151, 0.55)",
      },
    },
  },
  plugins: [],
};
