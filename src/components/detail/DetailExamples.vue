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
        <p v-if="!isBlank(ex.note)" class="ex-note">{{ ex.note }}</p>
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
  border-left: 3px solid color-mix(in srgb, var(--accent) 55%, var(--border));
  border-radius: var(--radius);
  background: var(--panel-inset);
}
.ex-head {
  display: flex;
  align-items: center;
  gap: 8px;
}
.ex-ko {
  font-size: 14px;
  font-weight: 650;
  color: var(--text);
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
  background: color-mix(in srgb, var(--accent) 12%, var(--panel));
  border-color: color-mix(in srgb, var(--accent) 45%, var(--border));
}
.ex-zh {
  margin: 4px 0 0;
  font-size: 12.5px;
  line-height: 1.6;
  color: var(--text-muted);
}
.ex-note {
  margin: 3px 0 0;
  font-size: 11.5px;
  line-height: 1.6;
  color: var(--text-faint);
}
</style>
