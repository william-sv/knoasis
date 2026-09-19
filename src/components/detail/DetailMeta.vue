<script setup>
// 详情 · 头部元数据条（v11）：词类(pos) / 形态细类(type) / 句式层级(speechLevel) / 等级 / 别名 / 出处 / 分类 / ID
// 优先读 view（content 透传，导入器已把条目级字段并入 content），回退 entry。
// 等级 label 由 set.levels 按 code 反查（entry.level 仅含 code/rank）。
import { computed } from "vue";

const props = defineProps({
  view: { type: Object, default: null },
  entry: { type: Object, default: null },
  set: { type: Object, default: null },
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

const pos = computed(() => fromView("pos") || fromEntry("pos"));
const typeFine = computed(() => fromView("type") || fromEntry("type"));
const speechLevel = computed(() => fromView("speechLevel") || fromEntry("speechLevel"));

// 等级 label：优先 set.levels 按 code 反查，回退 entry.level.label / code
const levelLabel = computed(() => {
  const e = props.entry || {};
  const lvl = e.level || {};
  const code = lvl.code || "";
  if (props.set && Array.isArray(props.set.levels)) {
    const hit = props.set.levels.find((l) => l.code === code);
    if (hit && hit.label) return hit.label;
  }
  return lvl.label || code || "";
});

const aliases = computed(() => {
  const a = fromView("aliases") || fromEntry("aliases");
  if (Array.isArray(a)) return a.filter((x) => !isEmpty(x));
  return a ? [String(a)] : [];
});

// sources: v11 [{type,ref}] → "type: ref"；兼容旧字符串形式
const sources = computed(() => {
  const s = fromView("sources") || fromEntry("sources");
  if (!Array.isArray(s)) return s ? [String(s)] : [];
  return s
    .map((x) => {
      if (x && typeof x === "object") {
        const type = !isEmpty(x.type) ? String(x.type) : "";
        const ref = !isEmpty(x.ref) ? String(x.ref) : "";
        return [type, ref].filter(Boolean).join(": ");
      }
      return isEmpty(x) ? "" : String(x);
    })
    .filter(Boolean);
});

const category = computed(() => fromEntry("category") || fromView("category"));
// 稳定 ID：优先 v11 数据集 id（ko…），回退 entry.uid
const entryId = computed(
  () => fromView("id") || (props.entry && (props.entry.uid || props.entry.id)) || "",
);

const show = computed(() =>
  Boolean(
    pos.value ||
      typeFine.value ||
      speechLevel.value ||
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
    <div v-if="pos || typeFine || speechLevel || levelLabel" class="dmeta-chips">
      <span v-if="pos" class="dm-chip dm-pos">{{ pos }}</span>
      <span v-if="typeFine" class="dm-chip dm-type">{{ typeFine }}</span>
      <span v-if="speechLevel" class="dm-chip dm-speech">{{ speechLevel }}</span>
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
.dm-type {
  color: var(--text-muted);
}
.dm-speech {
  color: var(--text);
  border-color: color-mix(in srgb, var(--accent) 30%, var(--border));
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
