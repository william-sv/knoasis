import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import "./styles/main.css";

// KaTeX 数学公式渲染（vue3-katex）：注册 v-katex 指令与 <katex> 组件，并引入排版样式
import "katex/dist/katex.min.css";
import katexPlugin from "vue3-katex";

// Knoasis UI 入口
// 真实数据只由 Rust IPC 提供（桌面应用）；纯浏览器（npm run dev）为空态预览，
// 组件接口与视图结构保持一致。
createApp(App).use(createPinia()).use(katexPlugin).mount("#app");
