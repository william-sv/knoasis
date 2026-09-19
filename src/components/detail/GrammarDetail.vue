<script setup>
// 语法详情 · 通用模板渲染包装（kr_grammar / en_grammar）
// 顶部展示 usage_scene / notes 单值概览（kr 模板的字段表外概览），随后按模板 sections 遍历。
// en 模板无 usage_scene/notes 字段，overview 自动隐藏。
import { computed } from "vue";
import DetailSections from "./DetailSections.vue";
import DetailMeta from "./DetailMeta.vue";

const props = defineProps({
  entry: { type: Object, default: null },
  set: { type: Object, default: null },
  view: { type: Object, default: null },
});

const usageScene = computed(() => (props.view?.fields?.usage_scene || "").trim());
const notesText = computed(() => (props.view?.fields?.notes || "").trim());
const hasOverview = computed(() => Boolean(usageScene.value || notesText.value));
</script>

<template>
  <div class="grammar-detail">
    <div v-if="hasOverview" class="overview">
      <p v-if="usageScene" class="overview-line">
        <span class="ov-label">使用场景</span>
        <span class="ov-text">{{ usageScene }}</span>
      </p>
      <p v-if="notesText" class="overview-line">
        <span class="ov-label">备注</span>
        <span class="ov-text">{{ notesText }}</span>
      </p>
    </div>

    <DetailMeta :view="view" :entry="entry" :set="set" />

    <DetailSections
      :template="set ? set.template : null"
      :view="view"
      :entry-uid="entry ? entry.uid : ''"
    />
    <div class="detail-bottom-space"></div>
  </div>
</template>

<style scoped>
.grammar-detail {
  display: flex;
  flex-direction: column;
}
.overview {
  margin-top: 6px;
  display: flex;
  flex-direction: column;
  gap: 7px;
}
.overview-line {
  display: flex;
  gap: 10px;
  margin: 0;
  padding: 8px 12px;
  border-radius: var(--radius);
  background: var(--panel-inset);
  border: 1px solid var(--border-soft);
  font-size: 12.5px;
  line-height: 1.7;
}
.ov-label {
  flex: none;
  font-size: 11px;
  color: var(--text-faint);
  padding-top: 1px;
}
.ov-text {
  flex: 1;
  min-width: 0;
  color: var(--text-muted);
  word-break: break-word;
  white-space: pre-wrap;
}
.detail-bottom-space {
  height: 24px;
}
</style>
