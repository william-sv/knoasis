<script setup>
// 详情 · 整段讲解：渲染模板 sections[type=paragraph]（大段 OCR 讲解不适合字段表）
// pre-wrap 保留原文断行；一律文本插值（不用 v-html）；空白文本整节隐藏。
import { computed } from "vue";

const props = defineProps({
  label: { type: String, default: "" },
  text: { type: String, default: "" },
});

const hasText = computed(() => (props.text || "").trim().length > 0);
</script>

<template>
  <section v-if="hasText" class="psection">
    <h2 v-if="label" class="psection-title">{{ label }}</h2>
    <div class="paragraph-body">{{ text }}</div>
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
</style>
