<script setup>
// 详情 · 多值子表：渲染模板 sections[type=list]（meanings/usages/scenes/examples/cautions/collocations）
// 空数组由父级 DetailSections 整节隐藏；item 内空字段自动跳过该行。

defineProps({
  label: { type: String, default: "" },
  items: { type: Array, default: () => [] },
  itemFields: { type: Array, default: () => [] }, // [{ key, label }]
  headKey: { type: String, default: "" }, // 列表项“主行”字段（如 examples.ko、meanings.meaning_zh）
});

function isBlank(v) {
  if (v == null) return true;
  if (Array.isArray(v)) return v.every((x) => String(x ?? "").trim() === "");
  return String(v).trim() === "";
}

function display(v) {
  if (v == null) return "";
  if (Array.isArray(v)) return v.filter((x) => String(x ?? "").trim()).join("、");
  return String(v);
}
</script>

<template>
  <section class="lsection">
    <h2 v-if="label" class="lsection-title">{{ label }}</h2>
    <div class="list-items">
      <article v-for="(item, idx) in items" :key="idx" class="list-item">
        <template v-for="f in itemFields" :key="f.key">
          <div v-if="!isBlank(item[f.key])" class="item-field">
            <span v-if="f.key !== headKey" class="item-label">{{ f.label || f.key }}</span>
            <span
              class="item-value"
              :class="{ 'item-value--head': f.key === headKey, 'is-zh': f.key === 'zh' || f.key === 'meaning_zh' }"
            >{{ display(item[f.key]) }}</span>
          </div>
        </template>
      </article>
    </div>
  </section>
</template>

<style scoped>
.lsection {
  margin: 22px 0 0;
}
.lsection-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-muted);
  margin: 0 0 10px;
}
.list-items {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.list-item {
  padding: 10px 12px;
  border: 1px solid var(--border-soft);
  border-radius: var(--radius);
  background: var(--panel-inset);
}
.item-field {
  display: flex;
  align-items: baseline;
  gap: 10px;
  padding: 1px 0;
  font-size: 12.5px;
  line-height: 1.65;
}
.item-label {
  flex: none;
  min-width: 56px;
  color: var(--text-faint);
  font-size: 11px;
}
.item-value {
  flex: 1;
  min-width: 0;
  color: var(--text);
  word-break: break-word;
  white-space: pre-wrap;
}
.item-value--head {
  font-weight: 650;
  color: var(--text);
}
.item-value.is-zh {
  color: var(--text-muted);
}
</style>
