<script setup>
// 列表区工具条：显示当前视图条目计数 + 生效的属性筛选（详情页标签触发）+ 一键清除筛选。
import { computed } from "vue";
import { useKnowledgeSets } from "../stores/knowledgeSets.js";
import { useSearch } from "../stores/search.js";
import { typeLabel } from "../lib/format.js";

const ks = useKnowledgeSets();
const search = useSearch();

// 等级 code → 展示 label（先查当前可用学科包的 levels，再回落内置语义）
function levelLabelFor(code) {
  if (code == null || code === "") return "未分级";
  for (const s of ks.sets) {
    const lv = (s.levels || []).find((l) => l.code === code);
    if (lv) return lv.label;
  }
  if (code === "I") return "TOPIK I";
  if (code === "II") return "TOPIK II";
  return code;
}

// 当前生效筛选的徽标文案
const activeFilters = computed(() => {
  const f = search.filters;
  const out = [];
  if (f.level != null) out.push({ key: "level", text: `等级：${levelLabelFor(f.level)}` });
  if (f.type != null) out.push({ key: "type", text: `类型：${typeLabel(f.type)}` });
  if (f.category != null) out.push({ key: "category", text: `分类：${f.category}` });
  return out;
});

function clearAll() {
  search.clearFilters();
}
</script>

<template>
  <div class="list-toolbar">
    <div class="stats">共 {{ ks.visibleCount }} 个知识点</div>

    <!-- 生效筛选：条件回显 + 出口 -->
    <div v-if="activeFilters.length" class="filters">
      <span v-for="f in activeFilters" :key="f.key" class="filter-chip">{{ f.text }}</span>
      <button class="clear-filters" title="清除全部筛选" @click="clearAll">清除筛选</button>
    </div>
  </div>
</template>

<style scoped>
.list-toolbar {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 6px;
  padding: 9px 16px 10px;
  border-bottom: 1px solid var(--border-soft);
  flex: none;
}

.stats {
  font-size: 11.5px;
  color: var(--text-muted);
}

.filters {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 6px;
}

.filter-chip {
  display: inline-flex;
  align-items: center;
  height: 20px;
  padding: 0 8px;
  border-radius: 999px;
  font-size: 11px;
  line-height: 20px;
  color: var(--accent);
  background: rgba(var(--accent-rgb), 0.14);
  border: 1px solid rgba(var(--accent-rgb), 0.4);
  white-space: nowrap;
}

.clear-filters {
  height: 20px;
  padding: 0 8px;
  border-radius: 999px;
  font-size: 11px;
  line-height: 20px;
  color: var(--text-muted);
  background: transparent;
  border: 1px solid var(--border);
  transition: background 0.12s ease, color 0.12s ease, border-color 0.12s ease;
}
.clear-filters:hover {
  background: var(--panel-hover);
  color: var(--text);
  border-color: rgba(var(--accent-rgb), 0.4);
}
</style>
