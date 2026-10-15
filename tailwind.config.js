/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{vue,js,ts,jsx,tsx}"],
  theme: {
    extend: {
      colors: {
        ink: {
          900: "#0b0e14",
          800: "#11151f",
          700: "#171c29",
          600: "#1f2635",
          500: "#2a3345",
        },
        jade: {
          400: "#38d9a9",
          500: "#20c997",
          600: "#12b886",
        },
        gold: {
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
        mono: ["JetBrains Mono", "Menlo", "monospace"],
      },
      boxShadow: {
        glow: "0 0 24px -6px rgba(32, 201, 151, 0.55)",
      },
    },
  },
  plugins: [],
};
