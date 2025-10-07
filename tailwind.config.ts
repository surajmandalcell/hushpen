import type { Config } from "tailwindcss";
import forms from "@tailwindcss/forms";
import typography from "@tailwindcss/typography";

const config: Config = {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx,css,html,vue,svelte}"],
  theme: {
    extend: {
      colors: {
        // Keep existing macos colors for backward compatibility
        macos: {
          bg: "#f5f5f7",
          "bg-dark": "#1d1d1f",
          sidebar: "#e8e8ed",
          "sidebar-dark": "#2c2c2e",
          border: "#d2d2d7",
          "border-dark": "#38383a",
          text: "#1d1d1f",
          "text-dark": "#f5f5f7",
          "text-secondary": "#86868b",
          blue: "#007aff",
          "blue-hover": "#0051d0",
          red: "#ff3b30",
          green: "#30d158",
          orange: "#ff9500",
          yellow: "#ffcc02",
          purple: "#af52de",
          pink: "#ff2d92",
          gray: {
            50: "#fafafa",
            100: "#f5f5f5",
            200: "#e5e5e5",
            300: "#d4d4d4",
            400: "#a3a3a3",
            500: "#737373",
            600: "#525252",
            700: "#404040",
            800: "#262626",
            900: "#171717",
          },
        },
        // New PalantirUI color palette
        palantir: {
          // Zinc/gray base palette
          zinc: {
            50: '#fafafa',
            100: '#f4f4f5',
            200: '#e4e4e7',
            300: '#d4d4d8',
            400: '#a1a1aa',
            500: '#71717a',
            600: '#52525b',
            700: '#3f3f46',
            800: '#27272a',
            900: '#18181b',
            950: '#09090b',
          },
          // Transparent layers
          layer: {
            1: 'rgba(250, 250, 250, 0.05)',
            2: 'rgba(250, 250, 250, 0.1)',
            3: 'rgba(250, 250, 250, 0.15)',
            4: 'rgba(250, 250, 250, 0.2)',
            5: 'rgba(250, 250, 250, 0.25)',
          },
          // Accent colors adapted for ASR
          accent: {
            blue: '#3b82f6',
            cyan: '#06b6d4',
            green: '#10b981',
            orange: '#f97316',
            red: '#ef4444',
          },
          // Glow effects
          glow: {
            subtle: '0 0 20px rgba(59, 130, 246, 0.15)',
            medium: '0 0 40px rgba(59, 130, 246, 0.25)',
            strong: '0 0 60px rgba(59, 130, 246, 0.35)',
          },
          // Dark mode variants
          'dark-layer': {
            1: 'rgba(9, 9, 11, 0.05)',
            2: 'rgba(9, 9, 11, 0.1)',
            3: 'rgba(9, 9, 11, 0.15)',
            4: 'rgba(9, 9, 11, 0.2)',
            5: 'rgba(9, 9, 11, 0.25)',
          },
        },
      },
      fontFamily: {
        // Keep existing SF font for backward compatibility
        sf: [
          "-apple-system",
          "BlinkMacSystemFont",
          "SF Pro Display",
          "SF Pro Text",
          "system-ui",
          "sans-serif",
        ],
        // New PalantirUI typography
        inter: ['Inter', 'system-ui', 'sans-serif'],
        ibm: ['IBM Plex Mono', 'Consolas', 'monospace'],
        kaisei: ['Kaisei Tokumin', 'serif'],
      },
      borderRadius: {
        // Keep existing macOS border radius for backward compatibility
        "macos-sm": "4px",
        macos: "6px",
        "macos-lg": "10px",
        "macos-xl": "12px",
        // New PalantirUI border radius
        "pal-sm": "2px",
        "pal": "4px",
        "pal-lg": "6px",
        "pal-xl": "8px",
      },
      boxShadow: {
        // Keep existing macOS shadows for backward compatibility
        macos: "0 1px 3px rgba(0, 0, 0, 0.12), 0 1px 2px rgba(0, 0, 0, 0.24)",
        "macos-lg":
          "0 4px 6px rgba(0, 0, 0, 0.07), 0 1px 3px rgba(0, 0, 0, 0.06)",
        "macos-xl":
          "0 10px 15px -3px rgba(0, 0, 0, 0.1), 0 4px 6px -2px rgba(0, 0, 0, 0.05)",
        "macos-inset": "inset 0 2px 4px 0 rgba(0, 0, 0, 0.06)",
        // New PalantirUI shadows
        "pal-subtle": "0 1px 3px rgba(0, 0, 0, 0.05), 0 1px 2px rgba(0, 0, 0, 0.1)",
        "pal-medium": "0 4px 6px rgba(0, 0, 0, 0.07), 0 2px 4px rgba(0, 0, 0, 0.06)",
        "pal-strong": "0 10px 15px rgba(0, 0, 0, 0.1), 0 4px 6px rgba(0, 0, 0, 0.05)",
        "pal-inset": "inset 0 1px 3px rgba(0, 0, 0, 0.1)",
        // Glow effects
        "pal-glow-subtle": "0 0 20px rgba(59, 130, 246, 0.15)",
        "pal-glow-medium": "0 0 40px rgba(59, 130, 246, 0.25)",
        "pal-glow-strong": "0 0 60px rgba(59, 130, 246, 0.35)",
      },
      backdropBlur: {
        // Keep existing macOS backdrop blur for backward compatibility
        macos: "20px",
        // New PalantirUI backdrop blur
        "pal-sm": "4px",
        "pal": "8px",
        "pal-lg": "12px",
        "pal-xl": "16px",
        "pal-2xl": "20px",
      },
      animation: {
        // Keep existing animations for backward compatibility
        "fade-in": "fadeIn 0.2s ease-out",
        "slide-up": "slideUp 0.3s ease-out",
        "slide-down": "slideDown 0.3s ease-out",
        "bounce-in": "bounceIn 0.4s cubic-bezier(0.68, -0.55, 0.265, 1.55)",
        "pulse-soft": "pulseSoft 2s cubic-bezier(0.4, 0, 0.6, 1) infinite",
        // New PalantirUI animations
        "pal-fade-in-up": "palFadeInUp 0.3s ease-out",
        "pal-fade-in": "palFadeIn 0.2s ease-out",
        "pal-card-fade-in": "palCardFadeIn 0.4s ease-out",
        "pal-glow-pulse": "palGlowPulse 2s ease-in-out infinite",
        "pal-corner-appear": "palCornerAppear 0.5s ease-out",
        "pal-border-glow": "palBorderGlow 3s ease-in-out infinite",
        "pal-slide-up": "palSlideUp 0.3s ease-out",
        "pal-scale-in": "palScaleIn 0.2s ease-out",
      },
      keyframes: {
        // Keep existing keyframes for backward compatibility
        fadeIn: {
          "0%": { opacity: "0" },
          "100%": { opacity: "1" },
        },
        slideUp: {
          "0%": { transform: "translateY(10px)", opacity: "0" },
          "100%": { transform: "translateY(0)", opacity: "1" },
        },
        slideDown: {
          "0%": { transform: "translateY(-10px)", opacity: "0" },
          "100%": { transform: "translateY(0)", opacity: "1" },
        },
        bounceIn: {
          "0%": { transform: "scale(0.3)", opacity: "0" },
          "50%": { transform: "scale(1.05)" },
          "70%": { transform: "scale(0.9)" },
          "100%": { transform: "scale(1)", opacity: "1" },
        },
        pulseSoft: {
          "0%, 100%": { opacity: "1" },
          "50%": { opacity: "0.7" },
        },
        // New PalantirUI keyframes
        palFadeInUp: {
          "0%": { transform: "translateY(20px)", opacity: "0" },
          "100%": { transform: "translateY(0)", opacity: "1" },
        },
        palFadeIn: {
          "0%": { opacity: "0" },
          "100%": { opacity: "1" },
        },
        palCardFadeIn: {
          "0%": { transform: "translateY(10px) scale(0.95)", opacity: "0" },
          "100%": { transform: "translateY(0) scale(1)", opacity: "1" },
        },
        palGlowPulse: {
          "0%, 100%": { boxShadow: "0 0 20px rgba(59, 130, 246, 0.15)" },
          "50%": { boxShadow: "0 0 40px rgba(59, 130, 246, 0.35)" },
        },
        palCornerAppear: {
          "0%": { opacity: "0", transform: "scale(0.8)" },
          "100%": { opacity: "0.7", transform: "scale(1)" },
        },
        palBorderGlow: {
          "0%, 100%": { borderColor: "rgba(59, 130, 246, 0.3)" },
          "50%": { borderColor: "rgba(59, 130, 246, 0.8)" },
        },
        palSlideUp: {
          "0%": { transform: "translateY(15px)", opacity: "0" },
          "100%": { transform: "translateY(0)", opacity: "1" },
        },
        palScaleIn: {
          "0%": { transform: "scale(0.9)", opacity: "0" },
          "100%": { transform: "scale(1)", opacity: "1" },
        },
      },
    },
  },
  plugins: [forms, typography],
  darkMode: "class",
};

export default config;