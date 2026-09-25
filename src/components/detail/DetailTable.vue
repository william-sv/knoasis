<script setup>
// 详情 · 通用二维子表（connections / patterns / similar 等）
// columns: [{ key, label, mono? }]；mono 列（韩文形态）用等宽强调。
import { computed } from "vue";

const props = defineProps({
  label: { type: String, default: "" },
  columns: { type: Array, default: () => [] }, // [{ key, label, mono? }]
  rows: { type: Array, default: () => [] },
});

function isBlank(v) {
  if (v == null) return true;
  if (Array.isArray(v)) return v.every((x) => String(x ?? "").trim() === "");
  return String(v).trim() === "";
}
function cell(row, col) {
  const v = row ? row[col.key] : null;
  if (v == null) return "";
  if (Array.isArray(v)) return v.filter((x) => String(x ?? "").trim()).join("、");
  return String(v);
}

const hasRows = computed(() => Array.isArray(props.rows) && props.rows.length > 0);
// 过滤掉全空行
const shownRows = computed(() =>
  hasRows.value ? props.rows.filter((r) => props.columns.some((c) => !isBlank(r[c.key]))) : [],
);
</script>

<template>
  <section v-if="shownRows.length" class="tsection">
    <h2 v-if="label" class="tsection-title">{{ label }}</h2>
    <div class="table-wrap">
      <table class="data-table">
        <thead>
          <tr>
            <th v-for="c in columns" :key="c.key" :class="{ 'col-mono': c.mono }">{{ c.label }}</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="(row, i) in shownRows" :key="i">
            <td
              v-for="c in columns"
              :key="c.key"
              :class="{ 'col-mono': c.mono, 'is-zh': c.key === 'zh' || c.key === 'difference' }"
            >{{ cell(row, c) }}</td>
          </tr>
        </tbody>
      </table>
    </div>
  </section>
</template>

<style scoped>
.tsection {
  margin: 22px 0 0;
}
.tsection-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-muted);
  margin: 0 0 10px;
}
.table-wrap {
  border: 1px solid var(--border-soft);
  border-radius: var(--radius);
  overflow: hidden;
  background: var(--panel);
}
.data-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12.5px;
  line-height: 1.6;
}
.data-table th,
.data-table td {
  text-align: left;
  padding: 8px 12px;
  border-bottom: 1px solid var(--border-soft);
  vertical-align: top;
  word-break: break-word;
}
.data-table th {
  background: var(--panel-inset);
  color: var(--text-faint);
  font-weight: 600;
  font-size: 11px;
  white-space: nowrap;
}
.data-table tbody tr:last-child td {
  border-bottom: none;
}
.data-table tbody tr:nth-child(even) {
  background: rgba(var(--panel-inset-rgb), 0.45);
}
.col-mono {
  font-family: var(--font-mono, monospace);
  color: var(--text);
  white-space: nowrap;
}
.is-zh {
  color: var(--text-muted);
}
</style>
