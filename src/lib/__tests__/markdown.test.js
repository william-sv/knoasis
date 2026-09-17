import { test } from "node:test";
import assert from "node:assert/strict";
import { renderMarkdown, renderRichHtml } from "../markdown.js";

test("有序列表 + 段落被解析为 ol/p", () => {
  const md = [
    "1. S+V ｜ 主语 + 动词",
    "2. S+V+O ｜ 主语 + 动词 + 宾语 ｜ S：主语",
    "",
    "虽然从初中开始就教五种基本句型。",
  ].join("\n");
  const html = renderMarkdown(md);
  assert.ok(html.includes("<ol>"), "应有有序列表");
  assert.ok(html.includes("<li>S+V ｜ 主语 + 动词</li>"), "第一条");
  assert.ok(html.includes("<li>S+V+O ｜ 主语 + 动词 + 宾语 ｜ S：主语</li>"), "第二条");
  assert.ok(html.includes("<p>虽然从初中开始就教五种基本句型。</p>"), "段落");
});

test("无序列表被解析为 ul", () => {
  const html = renderMarkdown("- a\n- b");
  assert.ok(html.includes("<ul>"), "应有无序列表");
  assert.ok(html.includes("<li>a</li>") && html.includes("<li>b</li>"));
});

test("原生 HTML 被转义（防注入）", () => {
  const html = renderMarkdown("<script>alert(1)</script>");
  assert.doesNotMatch(html, /<script>/);
  assert.match(html, /&lt;script&gt;/);
});

test("加粗与行内代码", () => {
  const html = renderMarkdown("**重点** 与 `code`");
  assert.match(html, /<strong>重点<\/strong>/);
  assert.match(html, /<code>code<\/code>/);
});

test("空串返回空", () => {
  assert.equal(renderMarkdown(""), "");
  assert.equal(renderMarkdown(null), "");
});

// —— Phase R：markdown-it + KaTeX 集成 ——

test("行内 LaTeX 渲染为 KaTeX（并消费 $ 分隔符）", () => {
  const html = renderMarkdown("质能方程 $E=mc^2$ 成立");
  assert.match(html, /class="katex"/, "应含 katex 渲染结果");
  assert.doesNotMatch(html, /\$E=mc\^2\$/, "原始 $...$ 分隔符应被消费");
});

test("块级 LaTeX 渲染为 KaTeX 块", () => {
  const html = renderMarkdown("$$\\int_0^1 x\\,dx$$");
  assert.match(html, /class="katex"/, "应含 katex 渲染结果");
  assert.match(html, /katex-block|katex-display/, "块级公式应有 katex-block/katex-display 容器");
});

test("富文本 format=html 保留原生标签（html:true）", () => {
  const html = renderRichHtml("<b>粗体</b> 与 <i>斜体</i>");
  assert.match(html, /<b>粗体<\/b>/, "原生 <b> 应保留");
  assert.match(html, /<i>斜体<\/i>/, "原生 <i> 应保留");
});
