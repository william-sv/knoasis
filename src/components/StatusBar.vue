<script setup>
// 底部状态栏
import { computed } from "vue";
import { useUi } from "../stores/ui.js";
import { useSearch } from "../stores/search.js";
import { useKnowledgeSets } from "../stores/knowledgeSets.js";
import { setDisplayName } from "../lib/format.js";

const ui = useUi();
const search = useSearch();
const ks = useKnowledgeSets();

const scopeText = computed(() => setDisplayName(ui.activeSetId));

const searchText = computed(() => {
  if (!search.active) return null;
  return `搜索“${search.q.trim()}” · ${search.hits.length} 个命中${search.tookMs ? ` · ${search.tookMs}ms` : ""}`;
});

// 生效筛选维度数量（等级/类型/分类）
const filterCount = computed(() => {
  const f = search.filters;
  return [f.level, f.type, f.category].filter((v) => v != null).length;
});
</script>

<template>
  <footer class="status-bar">
    <div class="status-left">
      <span>诺西斯</span>
    </div>
    <div class="status-right">
      <span v-if="searchText" class="status-search">{{ searchText }}</span>
      <span class="sep" v-if="searchText">·</span>
      <span v-if="filterCount" class="status-filter">筛选 {{ filterCount }} 项</span>
      <span class="sep" v-if="filterCount">·</span>
      <span class="status-count">共 {{ ks.visibleCount }} 条</span>
      <span class="sep">·</span>
      <span class="status-scope">当前：{{ scopeText }}</span>
    </div>
  </footer>
</template>

<style scoped>
.status-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  height: var(--status-h);
  padding: 0 14px;
  background: var(--panel);
  border-top: 1px solid var(--border);
  font-size: 11px;
  color: var(--text-muted);
  flex: none;
  white-space: nowrap;
}
.status-left,
.status-right {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  overflow: hidden;
}
.sep {
  color: var(--text-faint);
  opacity: 0.7;
}
.status-search {
  color: var(--accent);
}
.status-filter {
  color: var(--accent);
}
.status-count {
  color: var(--text-muted);
}
.status-scope {
  flex: none;
  color: var(--text-faint);
}
</style>
