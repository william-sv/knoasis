// Knoasis · Markdown 渲染（v2：markdown-it + KaTeX + DOMPurify 净化层）
//
// 引擎：markdown-it（原生覆盖段落 / 有序·无序列表 / 表格 / 加粗 / 行内代码等 CommonMark+GFM 子集）。
// 数学：@vscode/markdown-it-katex 插件（VS Code 团队维护，供应链安全 100），复用已装 katex 实例避免双份打包。
// 安全契约：
//   - markdown 实例 html:false → 解析层即转义原生 HTML（防注入），数学由插件产出受控 KaTeX HTML。
//   - html(富文本) 实例 html:true → 允许原生 HTML，但调用方（DetailParagraph）必须过 DOMPurify 再 v-html。
// 本模块只产出 HTML 字符串；净化统一放在视图层（浏览器 DOM 可用），便于单测与跨端复用。

import markdownIt from "markdown-it";
import katex from "katex";
import katexPlugin from "@vscode/markdown-it-katex";

// @vscode/markdown-it-katex 为 CJS 包，ESM 下 default 才是插件函数
const mk = katexPlugin.default || katexPlugin;

const katexOpts = { katex, throwOnError: false, errorColor: "#cc0000" };

// 默认实例：禁原生 HTML（防注入）
export const md = new markdownIt({
  html: false,
  linkify: true,
  breaks: false,
  typographer: false,
}).use(mk, katexOpts);

// 富文本实例：允许原生 HTML（必须经 DOMPurify 净化后使用）
export const mdHtml = new markdownIt({
  html: true,
  linkify: true,
  breaks: false,
  typographer: false,
}).use(mk, katexOpts);

// 标准 Markdown 渲染（段落讲解默认走这里）
export function renderMarkdown(src) {
  if (!src) return "";
  return md.render(String(src));
}

// 富文本渲染（format:"html"）：允许原生 HTML，调用方需 DOMPurify 净化
export function renderRichHtml(src) {
  if (!src) return "";
  return mdHtml.render(String(src));
}
