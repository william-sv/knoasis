<script setup>
// 详情 · 例句表（v8 新增 audio 占位字段）：韩文主行 + 中文 + 注 + 发音播放
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
  Array.isArray(props.items) ? props.items.filter((e) => !isBlank(e.ko)) : [],
);

// 释义 tag（examples[].note，如「顺序」「原因」）配色：按语义关键词取固定色，未命中走中性灰
const SENSE_COLORS = [
  { kw: ["原因", "理由", "因果"], c: "#7A3FA8", rgb: "122, 63, 168" }, // 紫
  { kw: ["顺序", "接续", "先后"], c: "#1F7A4C", rgb: "31, 122, 76" }, // 深绿
  { kw: ["转折", "对比", "逆接"], c: "#C0392B", rgb: "192, 57, 43" }, // 红
  { kw: ["条件", "假设"], c: "#2F6FB0", rgb: "47, 111, 176" }, // 蓝
  { kw: ["敬语", "谦敬", "尊敬"], c: "#D97706", rgb: "217, 119, 6" }, // 橙
  { kw: ["背景", "提示", "说明"], c: "#0E7490", rgb: "14, 116, 144" }, // 青
];
const SENSE_DEFAULT = { c: "#6B7280", rgb: "107, 114, 128" };
function senseStyle(note) {
  const t = String(note || "").trim();
  const hit = SENSE_COLORS.find((x) => x.kw.some((k) => t.includes(k)));
  const s = hit || SENSE_DEFAULT;
  return {
    color: s.c,
    borderColor: `rgba(${s.rgb}, 0.45)`,
    background: `rgba(${s.rgb}, 0.1)`,
  };
}

function play(audio) {
  if (!audio) return;
  try {
    const a = new Audio(audio);
    a.play().catch(() => {});
  } catch {
    /* 占位路径不可用时静默 */
  }
}
</script>

<template>
  <section v-if="shown.length" class="exsection">
    <h2 v-if="label" class="exsection-title">{{ label }}</h2>
    <div class="ex-items">
      <article v-for="(ex, i) in shown" :key="i" class="ex-item">
        <div class="ex-head">
          <span class="ex-ko">{{ ex.ko }}</span>
          <button
            v-if="ex.audio"
            type="button"
            class="ex-audio"
            title="播放发音"
            @click="play(ex.audio)"
          >
            <svg viewBox="0 0 24 24" width="13" height="13" fill="currentColor" aria-hidden="true">
              <path d="M8 5v14l11-7z" />
            </svg>
          </button>
        </div>
        <p v-if="!isBlank(ex.zh)" class="ex-zh">{{ ex.zh }}</p>
        <p v-if="!isBlank(ex.note)" class="ex-note">
          <span class="ex-sense" :style="senseStyle(ex.note)">{{ ex.note }}</span>
        </p>
      </article>
    </div>
  </section>
</template>

<style scoped>
.exsection {
  margin: 22px 0 0;
}
.exsection-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-muted);
  margin: 0 0 10px;
}
.ex-items {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.ex-item {
  padding: 10px 12px;
  border: 1px solid var(--border-soft);
  border-radius: var(--radius);
  background: var(--panel-inset);
}
.ex-head {
  display: flex;
  align-items: center;
  gap: 8px;
}
.ex-ko {
  font-size: 14.5px;
  font-weight: 700;
  color: var(--text);
  letter-spacing: 0.2px;
  word-break: break-word;
}
.ex-audio {
  flex: none;
  width: 22px;
  height: 22px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 1px solid var(--border);
  border-radius: 50%;
  background: var(--panel);
  color: var(--accent);
  cursor: pointer;
  transition: background 0.12s ease, border-color 0.12s ease;
}
.ex-audio:hover {
  background: rgba(var(--accent-rgb), 0.12);
  border-color: rgba(var(--accent-rgb), 0.45);
}
.ex-zh {
  margin: 4px 0 0;
  font-size: 12.5px;
  line-height: 1.6;
  color: var(--text-muted);
}
.ex-note {
  margin: 5px 0 0;
  font-size: 11.5px;
  line-height: 1.6;
}
/* 释义 tag：彩色胶囊（语义分色，显眼但不抢主句） */
.ex-sense {
  display: inline-block;
  padding: 1px 8px;
  border: 1px solid transparent;
  border-radius: 999px;
  font-size: 11px;
  font-weight: 600;
  line-height: 1.7;
}
</style>
