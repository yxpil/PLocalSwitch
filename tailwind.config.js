/** @type {import('tailwindcss').Config} */
// ============================================================
//  Tailwind 配置：圆角药丸 + 纯黑白双主题（零彩色）
// ============================================================
//  设计语言（B/W Pill Design）:
//    亮色：背景白 (#FFFFFF) / 中性灰分层 / 主色纯黑 (#000)
//    暗色：背景纯黑 (#000000) / 中性灰分层 / 主色纯白 (#FFF)
//    所有"语义"只用"黑/白/灰"三档（soft/medium/strong）表达
// ============================================================

export default {
  darkMode: 'class', // 由 .dark class 触发（与 themeStore 对齐）
  content: ['./index.html', './src/**/*.{ts,tsx,js,jsx}'],

  theme: {
    extend: {
      // ========== 圆角药丸 ==========
      borderRadius: {
        pill:    '9999px',
        soft:    '1rem',
        softer:  '1.5rem',
        softest: '2rem',
      },

      // ========== 配色：纯黑白 + 中性灰（零彩色） ==========
      colors: {
        // 主色（黑-白渐变语义），保持 primary/neutral 命名方便替换
        primary: {
          50:  '#fafafa',  // 极浅黑（≈白）
          100: '#f4f4f5',
          200: '#e4e4e7',
          300: '#d4d4d8',
          400: '#a1a1aa',
          500: '#71717a',
          600: '#52525b',
          700: '#3f3f46',
          800: '#27272a',
          900: '#18181b',
          950: '#0a0a0a',  // 纯黑近似
        },
        // 反转主色（供暗色模式当背景层用）
        ink: {
          DEFAULT: '#000000', // 纯黑
          paper:   '#ffffff', // 纯白
          soft:    '#111111', // 近乎纯黑
          hard:    '#f0f0f0', // 近乎纯白
        },
        // 语义：只用灰阶表达，不引入颜色
        // success → 深灰 + 边框，warning → 中灰，danger → 纯黑反白，info → 浅灰
        semantic: {
          pass:   { bg: '#111111', fg: '#ffffff', soft: '#f5f5f5', darkSoft: '#1f1f1f' },
          warn:   { bg: '#525252', fg: '#ffffff', soft: '#ececec', darkSoft: '#2a2a2a' },
          fail:   { bg: '#000000', fg: '#ffffff', soft: '#ffffff', darkSoft: '#171717', border: '#404040' },
          muted:  { bg: '#a1a1aa', fg: '#ffffff', soft: '#fafafa', darkSoft: '#151515' },
        },
      },

      // ========== 阴影：黑白（无彩）柔光阴影 ==========
      boxShadow: {
        // 药丸三态（纯黑灰调）
        'pill':         '0 2px 10px -3px rgba(0,0,0,0.12), 0 1px 3px rgba(0,0,0,0.04)',
        'pill-hover':   '0 12px 32px -8px rgba(0,0,0,0.18), 0 2px 6px rgba(0,0,0,0.06)',
        'pill-active':  '0 1px 3px rgba(0,0,0,0.15), inset 0 1px 2px rgba(255,255,255,0.15)',
        // 卡片
        'card':         '0 6px 24px -8px rgba(0,0,0,0.10), 0 1px 2px rgba(0,0,0,0.04)',
        'card-hover':   '0 18px 50px -14px rgba(0,0,0,0.16), 0 2px 6px rgba(0,0,0,0.05)',
        'soft':         '0 2px 12px -4px rgba(0,0,0,0.06)',
        // 暗色模式反色阴影
        'pill-dark':    '0 2px 10px -3px rgba(255,255,255,0.10), 0 1px 3px rgba(255,255,255,0.04)',
        'glow-dark':    '0 0 24px rgba(255,255,255,0.08)',
      },

      // ========== 字体 ==========
      fontFamily: {
        sans: ['"Inter"', '"PingFang SC"', '"Microsoft YaHei"', 'system-ui', 'sans-serif'],
        mono: ['"JetBrains Mono"', '"Fira Code"', 'Consolas', 'monospace'],
      },

      // ========== 动画 ==========
      transitionDuration: {
        DEFAULT: '220ms',
        PILL:    '280ms',
      },
      transitionTimingFunction: {
        PILL: 'cubic-bezier(0.4, 0, 0.2, 1)',
      },

      // ========== 背景渐变（纯黑白，无渐变：直白纯色） ==========
      backgroundImage: {
        // 亮色主药丸：纯黑（无渐变）
        'gradient-pill':         '#0a0a0a',
        // 亮色次药丸（反色）
        'gradient-pill-invert':  '#ffffff',
        // 暗色主药丸：纯白（无渐变）
        'gradient-pill-dark':    '#fafafa',
        // 背景分层（纯色）
        'gradient-light':        '#ffffff',
        'gradient-dark':         '#000000',
      },
    },
  },

  // ========== 插件：Pill 风格工具类（黑白语义） ==========
  plugins: [
    function ({ addUtilities, addComponents, theme }) {
      const util = {
        // 通用按钮基底
        '.pill-base': {
          borderRadius: '9999px',
          transition: 'all 280ms cubic-bezier(0.4, 0, 0.2, 1)',
          outline: 'none',
        },
        '.pill-btn': {
          borderRadius: '9999px',
          padding: '0.625rem 1.5rem',
          fontWeight: 500,
          transition: 'all 280ms cubic-bezier(0.4, 0, 0.2, 1)',
          display: 'inline-flex',
          alignItems: 'center',
          justifyContent: 'center',
          gap: '0.5rem',
          outline: 'none',
          userSelect: 'none',
          whiteSpace: 'nowrap',
          lineHeight: 1.2,
        },
        '.pill-input': {
          borderRadius: '9999px',
          padding: '0.625rem 1.25rem',
          transition: 'all 280ms cubic-bezier(0.4, 0, 0.2, 1)',
          outline: 'none',
          border: '1px solid transparent',
        },
        '.pill-card': {
          borderRadius: '1.5rem',
          padding: '1.5rem',
          transition: 'all 280ms cubic-bezier(0.4, 0, 0.2, 1)',
          background: '#ffffff',
          boxShadow: theme('boxShadow.card'),
        },
        '.dark .pill-card': {
          background: '#0a0a0a',
          border: '1px solid rgba(255,255,255,0.06)',
        },
      };
      addUtilities(util, ['responsive', 'hover', 'active', 'focus']);

      // 预设组件变体（纯黑白语义）
      const comp = {
        '.pill-variant-primary': {
          // 亮色：纯黑渐变 + 白字；暗色：纯白渐变 + 黑字
          background: theme('backgroundImage.gradient-pill'),
          color: '#ffffff',
          boxShadow: theme('boxShadow.pill'),
          '&:hover': {
            boxShadow: theme('boxShadow.pill-hover'),
            transform: 'translateY(-1px)',
          },
          '&:active': {
            boxShadow: theme('boxShadow.pill-active'),
            transform: 'translateY(0)',
          },
        },
        '.dark .pill-variant-primary': {
          background: theme('backgroundImage.gradient-pill-dark'),
          color: '#000000',
        },
        '.pill-variant-ghost': {
          background: 'transparent',
          color: '#27272a',
          border: '1px solid #e4e4e7',
          '&:hover': { background: '#f4f4f5' },
        },
        '.dark .pill-variant-ghost': {
          color: '#e4e4e7',
          border: '1px solid #3f3f46',
          '&:hover': { background: '#27272a' },
        },
        '.pill-variant-soft': {
          background: '#f4f4f5',
          color: '#18181b',
          border: '1px solid #e4e4e7',
          '&:hover': { background: '#e4e4e7' },
        },
        '.dark .pill-variant-soft': {
          background: '#18181b',
          color: '#e4e4e7',
          border: '1px solid #3f3f46',
          '&:hover': { background: '#27272a' },
        },
        '.pill-variant-danger': {
          // 纯黑 + 白字，加边框表达"强语义"，不引入红色
          background: '#000000',
          color: '#ffffff',
          border: '1px solid #000000',
          boxShadow: theme('boxShadow.pill'),
          '&:hover': { transform: 'translateY(-1px)', boxShadow: theme('boxShadow.pill-hover') },
        },
        '.dark .pill-variant-danger': {
          background: '#ffffff',
          color: '#000000',
          border: '1px solid #ffffff',
        },
      };
      addComponents(comp);
    },
  ],

  corePlugins: { preflight: true },
};
