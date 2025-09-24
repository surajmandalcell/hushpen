/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      // macOS-inspired colors and styling
      colors: {
        'macos': {
          'bg': '#f5f5f7',
          'bg-dark': '#1d1d1f',
          'sidebar': '#e8e8ed',
          'sidebar-dark': '#2c2c2e',
          'border': '#d2d2d7',
          'border-dark': '#38383a',
          'text': '#1d1d1f',
          'text-dark': '#f5f5f7',
          'text-secondary': '#86868b',
          'blue': '#007aff',
          'blue-hover': '#0051d0',
          'red': '#ff3b30',
          'green': '#30d158',
          'orange': '#ff9500',
          'yellow': '#ffcc02',
          'purple': '#af52de',
          'pink': '#ff2d92',
          'gray': {
            50: '#fafafa',
            100: '#f5f5f5',
            200: '#e5e5e5',
            300: '#d4d4d4',
            400: '#a3a3a3',
            500: '#737373',
            600: '#525252',
            700: '#404040',
            800: '#262626',
            900: '#171717',
          }
        }
      },
      fontFamily: {
        'sf': ['-apple-system', 'BlinkMacSystemFont', 'SF Pro Display', 'SF Pro Text', 'system-ui', 'sans-serif'],
      },
      borderRadius: {
        'macos': '6px',
        'macos-lg': '10px',
        'macos-xl': '12px',
      },
      boxShadow: {
        'macos': '0 1px 3px rgba(0, 0, 0, 0.12), 0 1px 2px rgba(0, 0, 0, 0.24)',
        'macos-lg': '0 4px 6px rgba(0, 0, 0, 0.07), 0 1px 3px rgba(0, 0, 0, 0.06)',
        'macos-xl': '0 10px 15px -3px rgba(0, 0, 0, 0.1), 0 4px 6px -2px rgba(0, 0, 0, 0.05)',
        'macos-inset': 'inset 0 2px 4px 0 rgba(0, 0, 0, 0.06)',
      },
      backdropBlur: {
        'macos': '20px',
      },
      animation: {
        'fade-in': 'fadeIn 0.2s ease-out',
        'slide-up': 'slideUp 0.3s ease-out',
        'slide-down': 'slideDown 0.3s ease-out',
        'bounce-in': 'bounceIn 0.4s cubic-bezier(0.68, -0.55, 0.265, 1.55)',
        'pulse-soft': 'pulseSoft 2s cubic-bezier(0.4, 0, 0.6, 1) infinite',
      },
      keyframes: {
        fadeIn: {
          '0%': { opacity: '0' },
          '100%': { opacity: '1' },
        },
        slideUp: {
          '0%': { transform: 'translateY(10px)', opacity: '0' },
          '100%': { transform: 'translateY(0)', opacity: '1' },
        },
        slideDown: {
          '0%': { transform: 'translateY(-10px)', opacity: '0' },
          '100%': { transform: 'translateY(0)', opacity: '1' },
        },
        bounceIn: {
          '0%': { transform: 'scale(0.3)', opacity: '0' },
          '50%': { transform: 'scale(1.05)' },
          '70%': { transform: 'scale(0.9)' },
          '100%': { transform: 'scale(1)', opacity: '1' },
        },
        pulseSoft: {
          '0%, 100%': { opacity: '1' },
          '50%': { opacity: '0.7' },
        },
      },
    },
  },
  plugins: [
    require('@tailwindcss/forms'),
    require('@tailwindcss/typography'),
  ],
  darkMode: 'class',
}