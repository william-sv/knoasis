<script setup>
// 详情 · 相关语法（v11 related 字符串数组）
//  - 内部引用（导入器已映射为 app uid "kr-grammar:…"）：解析为条目 headword，点击跳转
//  - 外部参照（"ext:概念"）：去前缀展示为静态概念 chip
//  - 兜底：无法解析的内部引用原样展示（只读）
import { computed } from "vue";
import { useUi } from "../../stores/ui.js";
import { useKnowledgeSets } from "../../stores/knowledgeSets.js";

const props = defineProps({
  label: { type: String, default: "相关语法" },
  related: { type: Array, default: () => [] },
});

const ui = useUi();
const ks = useKnowledgeSets();

const items = computed(() => {
  const out = [];
  for (const raw of props.related || []) {
    const s = String(raw == null ? "" : raw).trim();
    if (!s) continue;
    if (s.startsWith("ext:")) {
      out.push({ kind: "ext", key: s, text: s.slice(4) });
      continue;
    }
    if (s.startsWith("ko-grammar:")) {
      const target = ks.entryByUid.get(s);
      out.push({
        kind: target ? "uid" : "dangling",
        key: s,
        uid: s,
        text: target ? target.name : s,
      });
      continue;
    }
    out.push({ kind: "raw", key: s, text: s });
  }
  return out;
});

function openRelated(it) {
  const target = ks.entryByUid.get(it.uid);
  if (target) ui.jumpToEntry(target);
  else ui.showToast("条目已不在当前数据中");
}
</script>

<template>
  <section v-if="items.length" class="relchips">
    <h2 v-if="label" class="relchips-title">{{ label }}</h2>
    <div class="rc-list">
      <button
        v-for="it in items"
        :key="it.key"
        class="rc-chip"
        :class="`rc-${it.kind}`"
        :disabled="it.kind !== 'uid'"
        :title="it.kind === 'ext' ? '外部概念' : ''"
        @click="openRelated(it)"
      >
        <span class="rc-text">{{ it.text }}</span>
        <span v-if="it.kind === 'uid'" class="rc-arrow">→</span>
      </button>
    </div>
  </section>
</template>

<style scoped>
.relchips {
  margin: 22px 0 0;
}
.relchips-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-muted);
  margin: 0 0 10px;
}
.rc-list {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
.rc-chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 28px;
  padding: 0 10px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: var(--panel);
  font-size: 12px;
  color: var(--text);
  transition: background 0.12s ease, border-color 0.12s ease, transform 0.12s ease;
}
.rc-chip:not(:disabled) {
  cursor: pointer;
}
.rc-chip:not(:disabled):hover {
  background: var(--panel-hover);
  border-color: rgba(var(--accent-rgb), 0.4);
  transform: translateY(-1px);
}
.rc-chip:disabled {
  cursor: default;
  opacity: 0.9;
}
.rc-text {
  font-weight: 600;
}
.rc-arrow {
  font-size: 11px;
  color: var(--text-faint);
}
.rc-chip:not(:disabled):hover .rc-arrow {
  color: var(--accent);
}
.rc-ext {
  border-style: dashed;
  color: var(--text-muted);
}
.rc-dangling,
.rc-raw {
  color: var(--text-faint);
}
</style>
