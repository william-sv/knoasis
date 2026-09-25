<script setup>
// 详情 · 关联语法（related）：已解析条目 chips（点击跳转）+ 未收录原文折叠
// 与模板 sections 中 type=list key=related 对应；为跳转体验单独实现，不走通用列表。
import { ref } from "vue";
import { useUi } from "../../stores/ui.js";
import { useKnowledgeSets } from "../../stores/knowledgeSets.js";

const props = defineProps({
  label: { type: String, default: "近似语法" },
  related: {
    type: Object,
    default: () => ({ resolved: [], pending: [] }),
  },
});

const ui = useUi();
const ks = useKnowledgeSets();
const pendingOpen = ref(false);

function openRelated(r) {
  const target = ks.entryByUid.get(r.uid);
  if (target) {
    ui.jumpToEntry(target);
  } else {
    ui.showToast("条目已不在当前数据中");
  }
}
</script>

<template>
  <section class="relsection">
    <h2 v-if="label" class="rel-title">{{ label }}</h2>

    <!-- 已解析：本集条目，点击跳转 -->
    <div v-if="related.resolved && related.resolved.length" class="rel-chips">
      <button
        v-for="r in related.resolved"
        :key="r.uid"
        class="rel-chip"
        :title="r.note || r.relation"
        @click="openRelated(r)"
      >
        <span class="rel-name">{{ r.headword }}</span>
        <span v-if="r.relation" class="rel-rel">{{ r.relation }}</span>
        <span class="rel-arrow">→</span>
      </button>
    </div>

    <!-- 未收录：折叠展示原文（只读），不做同名全文搜索降级 -->
    <div v-if="related.pending && related.pending.length" class="rel-pending">
      <button class="rel-pending-toggle" @click="pendingOpen = !pendingOpen">
        <span>另有 {{ related.pending.length }} 条未收录关联</span>
        <span class="chevron" :class="{ open: pendingOpen }">▼</span>
      </button>
      <ul v-if="pendingOpen" class="rel-pending-list">
        <li v-for="(p, i) in related.pending" :key="i" class="rel-pending-item">
          <span class="rp-text">{{ p.targetText }}</span>
          <span v-if="p.relation" class="rp-rel">{{ p.relation }}</span>
          <span v-if="p.note" class="rp-note">{{ p.note }}</span>
        </li>
      </ul>
    </div>
  </section>
</template>

<style scoped>
.relsection {
  margin: 22px 0 0;
}
.rel-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-muted);
  margin: 0 0 10px;
}
.rel-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
.rel-chip {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  height: 28px;
  padding: 0 10px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: var(--panel);
  font-size: 12px;
  color: var(--text);
  transition: background 0.12s ease, border-color 0.12s ease, transform 0.12s ease;
}
.rel-chip:hover {
  background: var(--panel-hover);
  border-color: rgba(var(--accent-rgb), 0.4);
  transform: translateY(-1px);
}
.rel-name {
  font-weight: 600;
}
.rel-rel {
  font-size: 10.5px;
  color: var(--text-faint);
}
.rel-arrow {
  font-size: 11px;
  color: var(--text-faint);
}
.rel-chip:hover .rel-arrow {
  color: var(--accent);
}

.rel-pending {
  margin-top: 12px;
}
.rel-pending-toggle {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  color: var(--text-muted);
  background: transparent;
  border: none;
  padding: 2px 0;
  cursor: pointer;
}
.rel-pending-toggle:hover {
  color: var(--text);
}
.rel-pending-toggle .chevron {
  font-size: 9px;
  transition: transform 0.15s ease;
}
.rel-pending-toggle .chevron.open {
  transform: rotate(180deg);
}
.rel-pending-list {
  margin: 8px 0 0;
  padding: 0;
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.rel-pending-item {
  display: flex;
  align-items: baseline;
  flex-wrap: wrap;
  gap: 6px;
  padding: 5px 8px;
  border-radius: var(--radius-sm);
  background: var(--panel-inset);
  font-size: 12px;
}
.rp-text {
  color: var(--text);
  font-weight: 550;
}
.rp-rel {
  font-size: 10.5px;
  color: var(--accent);
  background: rgba(var(--accent-rgb), 0.1);
  border-radius: 4px;
  padding: 0 5px;
}
.rp-note {
  font-size: 11px;
  color: var(--text-faint);
}
</style>
