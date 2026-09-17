<script setup>
// 详情 · 头部元数据条（v8 新增）：中文术语 / 词性(품사) / 语体(register) / 等级 / 别名 / 出处
// 优先读 view（content 透传，含导入器并入的 term_cn/type/register/aliases/sources），
// 回退到 entry（level 等条目级字段）。
import { computed } from "vue";

const props = defineProps({
  view: { type: Object, default: null },
  entry: { type: Object, default: null },
});

function isEmpty(v) {
  if (v == null) return true;
  if (Array.isArray(v)) return v.length === 0;
  return String(v).trim() === "";
}
function fromView(...keys) {
  const v = props.view || {};
  for (const k of keys) {
    if (!isEmpty(v[k])) return v[k];
  }
  return null;
}
function fromEntry(...keys) {
  const e = props.entry || {};
  for (const k of keys) {
    if (!isEmpty(e[k])) return e[k];
  }
  return null;
}

const termCn = computed(() => fromView("term_cn") || fromEntry("term_cn"));
const pos = computed(() => fromView("type") || fromEntry("type"));
const register = computed(() => fromView("register") || fromEntry("register"));
const levelLabel = computed(() => {
  const lvl = props.entry && props.entry.level;
  return lvl && lvl.label ? lvl.label : "";
});

const aliases = computed(() => {
  const a = fromView("aliases") || fromEntry("aliases");
  if (Array.isArray(a)) return a.filter((x) => !isEmpty(x));
  return a ? [String(a)] : [];
});
const sources = computed(() => {
  const s = fromView("sources") || fromEntry("sources");
  if (Array.isArray(s)) return s.filter((x) => !isEmpty(x));
  return s ? [String(s)] : [];
});
const category = computed(() => fromEntry("category") || fromView("category"));
// 稳定 ID：优先 entry.uid，回退 entry.id
const entryId = computed(() => {
  const e = props.entry || {};
  return e.uid || e.id || "";
});

const show = computed(
  () =>
    Boolean(
      termCn.value ||
        pos.value ||
        register.value ||
        levelLabel.value ||
        aliases.value.length ||
        sources.value.length ||
        category.value ||
        entryId.value,
    ),
);
</script>

<template>
  <div v-if="show" class="dmeta">
    <p v-if="termCn" class="dmeta-cn">{{ termCn }}</p>
    <div v-if="pos || register || levelLabel" class="dmeta-chips">
      <span v-if="pos" class="dm-chip dm-pos">{{ pos }}</span>
      <span v-if="register" class="dm-chip dm-reg">{{ register }}</span>
      <span v-if="levelLabel" class="dm-chip dm-level">{{ levelLabel }}</span>
    </div>
    <p v-if="aliases.length" class="dmeta-line">
      <span class="dm-label">别名</span>{{ aliases.join(" · ") }}
    </p>
    <p v-if="sources.length" class="dmeta-line">
      <span class="dm-label">出处</span>{{ sources.join(" · ") }}
    </p>
    <p v-if="category" class="dmeta-line">
      <span class="dm-label">分类</span>{{ category }}
    </p>
    <p v-if="entryId" class="dmeta-line dmeta-id">
      <span class="dm-label">ID</span>{{ entryId }}
    </p>
  </div>
</template>

<style scoped>
.dmeta {
  margin: 4px 0 0;
  display: flex;
  flex-direction: column;
  gap: 7px;
}
.dmeta-cn {
  margin: 0;
  font-size: 14px;
  font-weight: 600;
  color: var(--text-muted);
  letter-spacing: 0.3px;
}
.dmeta-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.dm-chip {
  height: 21px;
  padding: 0 9px;
  font-size: 11px;
  line-height: 21px;
  border-radius: 999px;
  border: 1px solid var(--border);
  background: var(--chip-bg);
  color: var(--text);
  white-space: nowrap;
}
.dm-pos {
  color: var(--accent);
  border-color: color-mix(in srgb, var(--accent) 45%, var(--border));
  background: color-mix(in srgb, var(--accent) 10%, var(--panel));
  font-weight: 600;
}
.dm-reg {
  color: var(--text-muted);
}
.dm-level {
  color: var(--text);
}
.dmeta-line {
  margin: 0;
  font-size: 11.5px;
  line-height: 1.6;
  color: var(--text-faint);
  word-break: break-word;
}
.dmeta-id {
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  font-size: 11px;
  opacity: 0.85;
}
.dm-label {
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
