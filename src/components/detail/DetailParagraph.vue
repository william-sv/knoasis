<script setup>
// 详情 · 整段讲解：渲染模板 sections[type=paragraph]
// 渲染策略由 section.format 决定（默认 markdown）：
//   - markdown（默认）：markdown-it 解析 → DOMPurify 净化 → v-html（解析层已转义原生 HTML，净化为纵深防御）
//   - plain：文本插值 + pre-wrap 原样保留断行
//   - html：markdown-it(html:true) 渲染富文本 → DOMPurify 净化（必做，防 XSS）→ v-html
import { computed } from "vue";
import DOMPurify from "dompurify";
import { renderMarkdown, renderRichHtml } from "../../lib/markdown.js";

const props = defineProps({
  label: { type: String, default: "" },
  text: { type: String, default: "" },
  format: { type: String, default: "markdown" }, // markdown(默认) | plain | html
});

const resolved = computed(() => props.format || "markdown");

// markdown / html → 渲染后经 DOMPurify 净化再 v-html；plain → 文本插值
const html = computed(() => {
  const src = props.text || "";
  if (resolved.value === "markdown") {
    return DOMPurify.sanitize(renderMarkdown(src));
  }
  if (resolved.value === "html") {
    // 富文本允许原生 HTML（html:true），必须过 DOMPurify 再注入 DOM
    return DOMPurify.sanitize(renderRichHtml(src));
  }
  return null; // plain
});

const hasText = computed(() => (props.text || "").trim().length > 0);
</script>

<template>
  <section v-if="hasText" class="psection">
    <h2 v-if="label" class="psection-title">{{ label }}</h2>
    <div v-if="html !== null" class="paragraph-body md" v-html="html"></div>
    <div v-else class="paragraph-body" v-text="text"></div>
  </section>
</template>

<style scoped>
.psection {
  margin: 22px 0 0;
}
.psection-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-muted);
  margin: 0 0 10px;
}
.paragraph-body {
  padding: 12px 14px;
  border: 1px solid var(--border-soft);
  border-left: 3px solid color-mix(in srgb, var(--accent) 55%, var(--border));
  border-radius: var(--radius);
  background: var(--panel-inset);
  font-size: 13px;
  line-height: 1.8;
  color: var(--text);
  white-space: pre-wrap;
  word-break: break-word;
  overflow-wrap: anywhere;
}
/* markdown 渲染：由内部标签承载结构，关闭 pre-wrap */
.paragraph-body.md {
  white-space: normal;
}
.paragraph-body.md p {
  margin: 0 0 10px;
}
.paragraph-body.md p:last-child {
  margin-bottom: 0;
}
.paragraph-body.md ol,
.paragraph-body.md ul {
  margin: 0 0 10px;
  padding-left: 1.5em;
}
.paragraph-body.md li {
  margin: 3px 0;
}
.paragraph-body.md code {
  font-family: var(--font-mono, monospace);
  background: color-mix(in srgb, var(--text) 8%, transparent);
  padding: 0 4px;
  border-radius: 4px;
}
</style>
