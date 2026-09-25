<script setup>
// 语法详情 · 通用模板渲染包装（kr_grammar / en_grammar）
// 顶部展示 usage_scene / notes 单值概览（kr 模板的字段表外概览），随后按模板 sections 遍历。
// en 模板无 usage_scene/notes 字段，overview 自动隐藏。
import { computed } from "vue";
import DetailSections from "./DetailSections.vue";

const props = defineProps({
  entry: { type: Object, default: null },
  set: { type: Object, default: null },
  view: { type: Object, default: null },
});

const usageScene = computed(() => (props.view?.fields?.usage_scene || "").trim());
const notesText = computed(() => (props.view?.fields?.notes || "").trim());
const hasOverview = computed(() => Boolean(usageScene.value || notesText.value));

// 底部元信息：出处 / 别名 / 分类（ID 不再展示；空值隐藏）
function isEmpty(v) {
  if (v == null) return true;
  if (Array.isArray(v)) return v.length === 0;
  return String(v).trim() === "";
}
function fromView(...keys) {
  const v = props.view || {};
  for (const k of keys) if (!isEmpty(v[k])) return v[k];
  return null;
}
function fromEntry(...keys) {
  const e = props.entry || {};
  for (const k of keys) if (!isEmpty(e[k])) return e[k];
  return null;
}
const bottomMeta = computed(() => {
  const aliasesRaw = fromView("aliases") || fromEntry("aliases");
  const aliases = Array.isArray(aliasesRaw)
    ? aliasesRaw.filter((x) => !isEmpty(x)).map(String)
    : aliasesRaw
      ? [String(aliasesRaw)]
      : [];
  const sourcesRaw = fromView("sources") || fromEntry("sources");
  const sources = Array.isArray(sourcesRaw)
    ? sourcesRaw
        .map((x) => {
          if (x && typeof x === "object") {
            return [!isEmpty(x.type) ? String(x.type) : "", !isEmpty(x.ref) ? String(x.ref) : ""]
              .filter(Boolean)
              .join(": ");
          }
          return isEmpty(x) ? "" : String(x);
        })
        .filter(Boolean)
    : sourcesRaw
      ? [String(sourcesRaw)]
      : [];
  const category = fromEntry("category") || fromView("category");
  return {
    show: Boolean(sources.length || aliases.length || category),
    sources,
    aliases,
    category: category || "",
  };
});
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

    <DetailSections
      :template="set ? set.template : null"
      :view="view"
      :entry-uid="entry ? entry.uid : ''"
    />

    <section v-if="bottomMeta.show" class="grammar-bottom-meta">
      <p v-if="bottomMeta.sources.length" class="gbm-line">
        <span class="gbm-label">出处</span>{{ bottomMeta.sources.join(" · ") }}
      </p>
      <p v-if="bottomMeta.aliases.length" class="gbm-line">
        <span class="gbm-label">别名</span>{{ bottomMeta.aliases.join(" · ") }}
      </p>
      <p v-if="bottomMeta.category" class="gbm-line">
        <span class="gbm-label">分类</span>{{ bottomMeta.category }}
      </p>
    </section>

    <div class="detail-bottom-space"></div>
  </div>
</template>

<style scoped>
.grammar-detail {
  display: flex;
  flex-direction: column;
  /* 正文区显式允许文本选择（避免 WKWebView 下继承到 none 导致无法选中复制） */
  user-select: text;
  -webkit-user-select: text;
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
/* 底部元信息（出处 / 别名 / 分类）：中性浅底卡片，与上方彩色模块区分 */
.grammar-bottom-meta {
  margin-top: 22px;
  padding: 12px 14px;
  border: 1px solid var(--border-soft);
  border-radius: var(--radius);
  background: var(--panel-inset);
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.gbm-line {
  margin: 0;
  font-size: 11.5px;
  line-height: 1.6;
  color: var(--text-faint);
  word-break: break-word;
}
.gbm-label {
  display: inline-block;
  min-width: 30px;
  margin-right: 6px;
  padding: 0 5px;
  border-radius: 4px;
  background: var(--chip-bg);
  color: var(--text-faint);
  font-size: 10.5px;
}
</style>
