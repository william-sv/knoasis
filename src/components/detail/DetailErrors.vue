<script setup>
// 详情 · 易错点（v8 common_errors）：误用 → 正用 + 说明
import { computed } from "vue";

const props = defineProps({
  label: { type: String, default: "" },
  items: { type: Array, default: () => [] },
});

function isBlank(v) {
  if (v == null) return true;
  return String(v).trim() === "";
}
const shown = computed(() =>
  Array.isArray(props.items)
    ? props.items.filter((e) => !isBlank(e.wrong) || !isBlank(e.right))
    : [],
);

// v11 common_errors 稀疏扩展字段（type/level/register，仅少量条目有）→ 有值即展示，不遗漏数据
function metaBadges(e) {
  const out = [];
  for (const k of ["type", "level", "register"]) {
    if (!isBlank(e && e[k])) out.push(String(e[k]));
  }
  return out;
}
</script>

<template>
  <section v-if="shown.length" class="esection">
    <h2 v-if="label" class="esection-title">{{ label }}</h2>
    <div class="err-items">
      <article v-for="(e, i) in shown" :key="i" class="err-item">
        <div class="err-row">
          <span class="err-wrong">{{ e.wrong }}</span>
          <span class="err-arrow">→</span>
          <span class="err-right">{{ e.right }}</span>
        </div>
        <p v-if="!isBlank(e.note)" class="err-note">{{ e.note }}</p>
        <div v-if="metaBadges(e).length" class="err-meta">
          <span v-for="(b, bi) in metaBadges(e)" :key="bi" class="err-badge">{{ b }}</span>
        </div>
      </article>
    </div>
  </section>
</template>

<style scoped>
.esection {
  margin: 22px 0 0;
}
.esection-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-muted);
  margin: 0 0 10px;
}
.err-items {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.err-item {
  padding: 10px 12px;
  border: 1px solid var(--border-soft);
  border-left: 3px solid color-mix(in srgb, #cc0000 55%, var(--border));
  border-radius: var(--radius);
  background: var(--panel-inset);
}
.err-row {
  display: flex;
  align-items: baseline;
  flex-wrap: wrap;
  gap: 8px;
  font-size: 13px;
  line-height: 1.6;
}
.err-wrong {
  color: #cc0000;
  text-decoration: line-through;
  text-decoration-color: color-mix(in srgb, #cc0000 60%, transparent);
}
.err-arrow {
  color: var(--text-faint);
}
.err-right {
  color: #1a7a3c;
  font-weight: 650;
}
.err-note {
  margin: 5px 0 0;
  font-size: 11.5px;
  line-height: 1.6;
  color: var(--text-faint);
}
.err-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 5px;
}
.err-badge {
  font-size: 10px;
  line-height: 16px;
  padding: 0 6px;
  border-radius: 4px;
  color: var(--text-faint);
  background: var(--chip-bg);
}
</style>
