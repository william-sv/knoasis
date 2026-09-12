<script setup>
// 详情 · 图解网格：渲染模板 sections[type=images]（view.images = [{rel, path}]）
// 每张 <img> 走 assetUrl(path)（Tauri asset 协议，本地懒加载）；
// hover 露出删除按钮 → 内联确认「删除这张图？」→ user.imageHide → 成功后本地从 view.images 移除；
// 失败 toast「删除失败，请重试」并保留该图（可重试）。
// 恢复入口本期无 UI（P2 预留：ipc.user.imageRestore({entry_uid, img_path}) 契约已就绪，重导/重同步不会复活已删图）。
import { computed, ref } from "vue";
import { user } from "../../lib/ipc.js";
import { assetUrl } from "../../lib/assets.js";
import { useUi } from "../../stores/ui.js";

const props = defineProps({
  label: { type: String, default: "图解" },
  images: { type: Array, default: () => [] }, // [{ rel, path }]
  entryUid: { type: String, default: "" },
});

const ui = useUi();

// 内联确认状态：confirmRel 非空 = 正在等待确认删除该 rel
const confirmRel = ref("");
const busy = ref(false);

function srcOf(img) {
  return assetUrl(img.path);
}

function beginRemove(img) {
  if (busy.value) return;
  confirmRel.value = img.rel;
}

function cancelRemove() {
  if (busy.value) return;
  confirmRel.value = "";
}

async function confirmRemove() {
  const rel = confirmRel.value;
  if (!rel || busy.value) return;
  busy.value = true;
  try {
    await user.imageHide({ entry_uid: props.entryUid, img_path: rel });
    // 成功 → 本地即时从可见视图移除该项（下次打开仍由 Rust get_detail 按 hidden 集过滤）
    const idx = props.images.findIndex((i) => i.rel === rel);
    if (idx >= 0) props.images.splice(idx, 1);
  } catch (e) {
    ui.showToast("删除失败，请重试", 2400);
  } finally {
    confirmRel.value = "";
    busy.value = false;
  }
}

const showRemove = computed(() => !busy.value);
</script>

<template>
  <section class="isection">
    <h2 v-if="label" class="isection-title">{{ label }}</h2>
    <div class="image-grid">
      <figure v-for="img in images" :key="img.rel" class="image-cell">
        <img v-if="srcOf(img)" :src="srcOf(img)" :alt="img.rel" class="image-img" loading="lazy" />
        <div v-else class="image-missing">图片不可用</div>

        <!-- hover 删除按钮 -->
        <button
          v-if="confirmRel !== img.rel && showRemove"
          class="remove-btn"
          title="删除这张图"
          @click="beginRemove(img)"
        >
          <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/><line x1="10" y1="11" x2="10" y2="17"/><line x1="14" y1="11" x2="14" y2="17"/></svg>
        </button>

        <!-- 内联确认 -->
        <div v-if="confirmRel === img.rel" class="confirm-bar" @click.stop>
          <span class="confirm-text">删除这张图？</span>
          <button class="confirm-btn confirm-btn--yes" :disabled="busy" @click="confirmRemove">
            确定
          </button>
          <button class="confirm-btn" :disabled="busy" @click="cancelRemove">取消</button>
        </div>
      </figure>
    </div>
  </section>
</template>

<style scoped>
.isection {
  margin: 22px 0 0;
}
.isection-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-muted);
  margin: 0 0 10px;
}
.image-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
  gap: 10px;
}
.image-cell {
  position: relative;
  margin: 0;
  border: 1px solid var(--border-soft);
  border-radius: var(--radius);
  overflow: hidden;
  background: var(--panel-inset);
}
.image-img {
  display: block;
  width: 100%;
  height: auto;
  min-height: 84px;
  object-fit: contain;
  background: var(--panel-inset);
}
.image-missing {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 84px;
  font-size: 11px;
  color: var(--text-faint);
}
.remove-btn {
  position: absolute;
  top: 6px;
  right: 6px;
  width: 26px;
  height: 26px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 1px solid color-mix(in srgb, #e5484d 45%, var(--border));
  border-radius: 6px;
  background: color-mix(in srgb, #ffffff 88%, transparent);
  color: #d93025;
  opacity: 0;
  transition: opacity 0.12s ease, background 0.12s ease;
  cursor: pointer;
}
.image-cell:hover .remove-btn {
  opacity: 1;
}
.remove-btn:hover {
  background: #fff;
}
.confirm-bar {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 8px;
  background: color-mix(in srgb, #ffffff 92%, transparent);
  border-top: 1px solid var(--border-soft);
  backdrop-filter: blur(2px);
}
.confirm-text {
  flex: 1;
  min-width: 0;
  font-size: 11px;
  color: var(--text);
}
.confirm-btn {
  height: 22px;
  padding: 0 8px;
  border: 1px solid var(--border);
  border-radius: 5px;
  background: var(--panel);
  font-size: 11px;
  color: var(--text-muted);
  cursor: pointer;
}
.confirm-btn:hover:not(:disabled) {
  color: var(--text);
}
.confirm-btn--yes {
  border-color: color-mix(in srgb, #e5484d 45%, var(--border));
  color: #d93025;
}
.confirm-btn--yes:hover:not(:disabled) {
  background: color-mix(in srgb, #e5484d 8%, var(--panel));
}
.confirm-btn:disabled {
  opacity: 0.5;
  cursor: default;
}
</style>
