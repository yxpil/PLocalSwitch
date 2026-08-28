// PostCSS 配置 - 与 Tailwind 构建链路对齐
// 仅在构建期运行，浏览器只拿到纯 CSS 产物
export default {
  plugins: {
    tailwindcss: {},
    autoprefixer: {},
  },
};
