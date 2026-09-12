<script setup>
// 详情 · 字段表：渲染模板 sections[type=fields] 中的非空字段
// 空字符串 / 空数组字段自动跳过；数组（variants/attaches_to）以「、」连接。
// 渲染一律文本插值（不用 v-html）。

defineProps({
  label: { type: String, default: "" },
  fields: { type: Array, default: () => [] }, // [{ key, label }]
  viewFields: { type: Object, default: () => ({}) }, // adapter.toDetailView().fields
});

function displayValue(f) {
  const v = f.value;
  if (v == null) return "";
  if (Array.isArray(v)) return v.filter((x) => String(x ?? "").trim()).join("、");
  if (typeof v === "boolean") return v ? "是" : "否";
  return String(v);
}

function isBlank(v) {
  if (v == null) return true;
  if (Array.isArray(v)) return v.every((x) => String(x ?? "").trim() === "");
  return String(v).trim() === "";
}
</script>

<template>
  <section class="fsection">
    <h2 v-if="label" class="fsection-title">{{ label }}</h2>
    <dl class="field-table">
      <template v-for="f in fields" :key="f.key">
        <div v-if="!isBlank(viewFields[f.key])" class="field-row">
          <dt class="field-label">{{ f.label || f.key }}</dt>
          <dd class="field-value">{{ displayValue({ value: viewFields[f.key] }) }}</dd>
        </div>
      </template>
    </dl>
  </section>
</template>

<style scoped>
.fsection {
  margin: 22px 0 0;
}
.fsection-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-muted);
  margin: 0 0 10px;
}
.field-table {
  display: flex;
  flex-direction: column;
  gap: 7px;
  margin: 0;
}
.field-row {
  display: flex;
  align-items: baseline;
  gap: 12px;
}
.field-label {
  flex: none;
  min-width: 86px;
  font-size: 12px;
  color: var(--text-faint);
  text-align: right;
}
.field-value {
  flex: 1;
  min-width: 0;
  font-size: 13px;
  line-height: 1.7;
  color: var(--text);
  word-break: break-word;
  white-space: pre-wrap;
}
</style>
