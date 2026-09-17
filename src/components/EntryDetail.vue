<script setup>
// 右列：条目详情
// 标题 + 元数据标签（右上角并列 收藏 / 复制 操作）+ 正文。
// 正文按数据形态分流：
//  - 模板驱动（学科包 set.template.sections 存在）：详情 knowledge_get_entry → toDetailView(content 透传)
//    → GrammarDetail 通用模板渲染（不再按 dto 字段做 kr/en 分支）
//  - html（旧内容型正文保留）：原 v-html + KaTeX 渲染（未来 sanitize 后正文）
// 笔记 textarea 展开于正文下方，输入即时保存（store 层防抖落 Rust）。
import { computed, reactive, ref, watch } from "vue";
import { useUi } from "../stores/ui.js";
import { useKnowledgeSets } from "../stores/knowledgeSets.js";
import { useFavorites } from "../stores/favorites.js";
import { useNotes } from "../stores/notes.js";
import { useSearch } from "../stores/search.js";
import { copyText, stripHtml, typeLabel, formatTime } from "../lib/format.js";
import { findSetById } from "../lib/data-registry.js";
import { knowledge as ipcKnowledge } from "../lib/ipc.js";
import { toDetailView } from "../lib/grammar/adapter.js";
import { exportGrammarMarkdown } from "../lib/grammar/export.js";
import GrammarDetail from "./detail/GrammarDetail.vue";
import copyIcon from "../assets/icons/copy.png";

const ui = useUi();
const ks = useKnowledgeSets();
const favorites = useFavorites();
const notesStore = useNotes();
const search = useSearch();

const entry = computed(() => ui.selectedEntry);

// ---------- 详情异步缓存（模板驱动详情） ----------
const detailViews = reactive(new Map()); // uid -> toDetailView(payload)（content JSON 透传 + images 规范化）
const detailLoading = reactive(new Map()); // uid -> bool

const activeSet = computed(() => (entry.value ? findSetById(entry.value.discipline) : null));
// 结构化详情判据：学科包 meta 提供 template.sections → 一律走通用模板渲染（不再按字段/dto 分支）
const isStructured = computed(() =>
  Boolean(
    activeSet.value &&
      activeSet.value.template &&
      Array.isArray(activeSet.value.template.sections) &&
      activeSet.value.template.sections.length,
  ),
);
const currentView = computed(() => (entry.value ? detailViews.get(entry.value.uid) : null));
const detailBusy = computed(() =>
  entry.value ? Boolean(detailLoading.get(entry.value.uid)) : false,
);

async function loadDetail(uid) {
  if (detailViews.get(uid)) return;
  if (detailLoading.get(uid)) return;
  detailLoading.set(uid, true);
  try {
    const payload = await ipcKnowledge.getEntry(uid);
    // content 已是 view-ready JSON（键形与模板 sections 对齐）；toDetailView 仅透传 + images 规范化
    detailViews.set(uid, toDetailView(payload));
  } catch (e) {
    if (e && (e.code === "ENTRY_NOT_FOUND" || e.code === "SET_NOT_FOUND")) {
      ui.showToast("条目已不在当前数据中", 2400);
    } else {
      ui.showToast("详情加载失败，请重试", 2400);
    }
  } finally {
    detailLoading.set(uid, false);
  }
}

// ---------- 笔记 ----------
const noteOpen = ref(false);
const noteText = ref("");
const savedAt = ref(null);

watch(entry, (e) => {
  if (!e) {
    noteOpen.value = false;
    noteText.value = "";
    savedAt.value = null;
    return;
  }
  const note = notesStore.getNote(e.uid);
  noteText.value = note ? note.content : "";
  savedAt.value = note ? note.updatedAt : null;
  if (isStructured.value && !detailViews.get(e.uid) && !detailLoading.get(e.uid)) {
    loadDetail(e.uid);
  }
});

function onNoteInput() {
  if (!entry.value) return;
  notesStore.saveNote(entry.value.uid, noteText.value);
  savedAt.value = notesStore.getNote(entry.value.uid).updatedAt;
}

function toggleNote() {
  if (!entry.value) return;
  noteOpen.value = !noteOpen.value;
  if (noteOpen.value && entry.value) {
    const note = notesStore.getNote(entry.value.uid);
    noteText.value = note ? note.content : "";
    savedAt.value = note ? note.updatedAt : null;
  }
}

// ---------- 收藏 / 复制 ----------
const isFav = computed(() => (entry.value ? favorites.isFavorite(entry.value.uid) : false));

function toggleFav() {
  if (!entry.value) return;
  const nowFav = favorites.toggle(entry.value);
  ui.showToast(nowFav ? `已收藏「${entry.value.name}」` : `已取消收藏「${entry.value.name}」`);
}

async function onCopy() {
  if (!entry.value) return;
  const e = entry.value;
  let ok = false;
  if (isStructured.value) {
    // 模板驱动详情（学科包通用）：确保详情就绪后导出 Markdown
    if (!currentView.value && !detailBusy.value) await loadDetail(e.uid);
    const view = currentView.value;
    const text = view
      ? exportGrammarMarkdown(e, activeSet.value, view)
      : `# ${e.name}\n${e.summary || ""}`;
    ok = await copyText(text);
  } else {
    const lines = [
      `# ${e.name}`,
      `${findSetById(e.discipline) ? findSetById(e.discipline).name : e.discipline} · ${typeLabel(e.type)}`,
      "",
      e.summary,
      "",
      "—— 正文 ——",
      stripHtml(e.content),
    ];
    ok = await copyText(lines.join("\n"));
  }
  ui.showToast(ok ? "已复制到剪贴板" : "复制失败，请手动选择");
}

// ---------- 相关知识点（legacy html 分支用） ----------
const relatedItems = computed(() => {
  if (!entry.value || !entry.value.related) return [];
  return entry.value.related
    .map((uid) => ks.entryByUid.get(uid))
    .filter(Boolean);
});

function jumpRelated(item) {
  ui.jumpToEntry(item);
}

// ---------- 顶部标签 → 联动筛选左侧列表 ----------
function toggleFilter(key, value) {
  search.setFilter(key, value);
}
function tagActive(t) {
  return entry.value != null && search.filters.tag === t;
}
function toggleTag(t) {
  search.setFilter("tag", t);
}

const noteSavedHint = computed(() => {
  if (!noteText.value && !savedAt.value) return "写点自己的理解，会自动保存到本地";
  return savedAt.value ? `已保存 · ${formatTime(savedAt.value)}` : "已自动保存";
});

// KaTeX 自动渲染配置：识别正文中的 $...$（行内）与 $$...$$（独立成块）
// 通过 vue3-katex 的 v-katex:auto 指令作用于 legacy 正文元素
const katexOptions = {
  options: {
    delimiters: [
      { left: "$$", right: "$$", display: true },
      { left: "$", right: "$", display: false },
    ],
  },
};
</script>

<template>
  <div class="detail">
    <!-- 无选中：占位空态 -->
    <div v-if="!entry" class="empty detail-empty">
      <div class="empty-title">还没有选择知识点</div>
      <div class="empty-hint">从左侧列表选择一个知识点，即可在此查看详细内容。</div>
    </div>

    <template v-else>
      <!-- 可滚动正文 -->
      <div class="detail-scroll">
        <div class="detail-head">
          <div class="meta-row">
            <div class="meta-tags">
              <span
                v-for="t in (entry.tags || [])"
                :key="t"
                class="chip chip--faint chip--clickable chip--tag"
                :class="{ 'is-active': tagActive(t) }"
                :title="tagActive(t) ? '取消标签筛选' : `只看标签「${t}」`"
                @click="toggleTag(t)"
              >{{ t }}</span>
            </div>
            <div class="meta-actions">
              <button
                class="icon-btn fav-btn"
                :class="{ 'is-fav': isFav }"
                :title="isFav ? '取消收藏' : '加入收藏'"
                @click="toggleFav"
              >
                <svg
                  class="star"
                  viewBox="0 0 24 24"
                  width="15"
                  height="15"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                >
                  <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" />
                </svg>
              </button>
              <button class="icon-btn copy-btn" :title="isStructured ? '复制为 Markdown' : '复制为纯文本'" @click="onCopy">
                <img :src="copyIcon" alt="复制" class="copy-icon" />
              </button>
            </div>
          </div>

          <h1 class="title">
            <span class="hash">#</span>
            <span class="title-text">{{ entry.name }}</span>
          </h1>

          <p v-if="entry.aliases && entry.aliases.length" class="aliases">
            <span class="alias-label">别名</span>
            <span v-for="(a, i) in entry.aliases" :key="a" class="alias-item">
              {{ i > 0 ? " · " : "" }}{{ a }}
            </span>
          </p>

          <p v-if="entry.summary" class="lead">{{ entry.summary }}</p>
          <p v-else-if="!isStructured" class="lead">…</p>
        </div>

        <!-- 模板驱动详情：学科包 template.sections 通用渲染（header/收藏/复制/笔记保留） -->
        <template v-if="isStructured">
          <div v-if="detailBusy && !currentView" class="detail-skeleton" aria-busy="true">
            <span class="sk sk-title"></span>
            <span class="sk sk-line"></span>
            <span class="sk sk-line short"></span>
            <span class="sk sk-block"></span>
            <span class="sk sk-block short"></span>
          </div>
          <GrammarDetail
            v-else-if="currentView"
            :entry="entry"
            :set="activeSet"
            :view="currentView"
          />
          <p v-else class="detail-error">详情暂不可用，请稍后重试。</p>
        </template>

        <!-- legacy：HTML 正文（内容型知识集的正文，未来 sanitize 后渲染） -->
        <template v-else>
          <article class="entry-content" v-html="entry.content" v-katex:auto="katexOptions"></article>

          <!-- 相关知识点 -->
          <section v-if="relatedItems.length" class="related">
            <h2 class="related-title">相关知识点</h2>
            <div class="related-chips">
              <button
                v-for="rel in relatedItems"
                :key="rel.uid"
                class="rel-chip"
                @click="jumpRelated(rel)"
              >
                <span
                  class="dot"
                  :style="{ background: findSetById(rel.discipline) ? findSetById(rel.discipline).color : 'var(--text-faint)' }"
                ></span>
                <span class="rel-name">{{ rel.name }}</span>
                <span class="rel-arrow">→</span>
              </button>
            </div>
          </section>

          <div class="detail-bottom-space"></div>
        </template>
      </div>

      <!-- 笔记（展开于正文下方·详情底部） -->
      <div v-if="noteOpen" class="note-panel">
        <div class="note-head">
          <span class="note-title">笔记</span>
          <span class="note-saved">{{ noteSavedHint }}</span>
        </div>
        <textarea
          v-model="noteText"
          class="note-area"
          rows="4"
          placeholder="记录你的理解、例句或疑问……（输入即自动保存）"
          spellcheck="false"
          @input="onNoteInput"
        ></textarea>
      </div>
    </template>
  </div>
</template>

<style scoped>
.detail {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-width: 0;
  background: var(--panel);
}
.detail-empty {
  background: var(--panel);
}

/* 滚动正文 */
.detail-scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 26px 34px 0;
  /* 正文区域显式允许文本选择（全局默认可选，这里再声明以覆盖 WKWebView 继承差异） */
  user-select: text;
  -webkit-user-select: text;
}
.detail-head {
  margin-bottom: 6px;
}
.meta-row {
  position: relative;
  min-height: 30px;
}
.meta-tags {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 6px;
  min-width: 0;
  /* 预留右上角 收藏/复制 按钮宽度，tags 自行换行时不挤压按钮 */
  padding-right: 78px;
}
.meta-actions {
  position: absolute;
  top: 0;
  right: 0;
  display: flex;
  align-items: center;
  gap: 6px;
  flex: none;
}
.chip {
  height: 21px;
  padding: 0 9px;
  font-size: 11px;
  line-height: 21px;
}
.chip--faint {
  color: var(--text-faint);
}
/* 详情页顶部 tags：可点击筛选（复用 chip--clickable 交互态） */
.chip--tag {
  display: inline-flex;
  align-items: center;
}
/* 可点击标签（学科/类型/等级/分类）：交互态 + 明显选中态 */
.chip--clickable {
  cursor: pointer;
  border: 1px solid transparent;
  transition: background 0.12s ease, color 0.12s ease, border-color 0.12s ease,
    box-shadow 0.12s ease;
}
.chip--clickable:hover {
  background: color-mix(in srgb, var(--accent) 14%, var(--chip-bg));
  color: var(--text);
}
.chip--clickable:focus-visible {
  outline: 2px solid color-mix(in srgb, var(--accent) 55%, transparent);
  outline-offset: 1px;
}
.chip--clickable.is-active {
  background: color-mix(in srgb, var(--accent) 20%, var(--panel));
  color: var(--accent);
  font-weight: 650;
  border-color: color-mix(in srgb, var(--accent) 55%, transparent);
  box-shadow: 0 1px 3px color-mix(in srgb, var(--accent) 22%, transparent);
}
.fav-btn {
  width: 30px;
  height: 30px;
  padding: 0;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: var(--text-muted);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--panel);
  transition: background 0.12s ease, border-color 0.12s ease, color 0.12s ease;
}
.fav-btn:hover {
  background: var(--panel-hover);
  color: var(--text);
}
.fav-btn .star {
  width: 15px;
  height: 15px;
  transition: fill 0.12s ease, stroke 0.12s ease;
}
.fav-btn.is-fav {
  color: #f5b301;
  border-color: color-mix(in srgb, #f5b301 45%, var(--border));
  background: color-mix(in srgb, #f5b301 12%, var(--panel));
}
.fav-btn.is-fav .star {
  fill: #f5b301;
  stroke: #f5b301;
}
.copy-btn {
  width: 30px;
  height: 30px;
  padding: 0;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: var(--text-muted);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--panel);
  transition: background 0.12s ease, border-color 0.12s ease, color 0.12s ease;
}
.copy-btn:hover {
  background: var(--panel-hover);
  color: var(--text);
  border-color: color-mix(in srgb, var(--accent) 40%, var(--border));
}
.copy-icon {
  width: 15px;
  height: 15px;
  object-fit: contain;
}

.title {
  display: flex;
  align-items: baseline;
  gap: 8px;
  margin: 14px 0 4px;
  font-size: 24px;
  font-weight: 700;
  letter-spacing: 0.2px;
  line-height: 1.3;
}
.hash {
  color: var(--text-faint);
  font-weight: 500;
}
.title-text {
  color: var(--text);
}

.aliases {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 4px;
  margin: 0 0 10px;
  font-size: 12px;
  color: var(--text-faint);
}
.alias-label {
  font-size: 10.5px;
  padding: 0 5px;
  border-radius: 4px;
  background: var(--chip-bg);
  color: var(--text-faint);
  margin-right: 2px;
}

.lead {
  margin: 0 0 10px;
  padding: 9px 12px;
  font-size: 13px;
  line-height: 1.7;
  color: var(--text-muted);
  background: var(--panel-inset);
  border: 1px solid var(--border-soft);
  border-radius: var(--radius);
}

/* 相关知识点（legacy） */
.related {
  margin-top: 26px;
  padding-top: 16px;
  border-top: 1px solid var(--border-soft);
}
.related-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-muted);
  margin: 0 0 10px;
}
.related-chips {
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
  border-color: color-mix(in srgb, var(--accent) 40%, var(--border));
  transform: translateY(-1px);
}
.rel-chip .dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex: none;
}
.rel-arrow {
  font-size: 11px;
  color: var(--text-faint);
}
.rel-chip:hover .rel-arrow {
  color: var(--accent);
}
.detail-bottom-space {
  height: 24px;
}

/* grammar 详情骨架 */
.detail-skeleton {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 16px 0;
}
.sk {
  display: block;
  height: 12px;
  border-radius: 6px;
  background: linear-gradient(90deg, var(--panel-inset) 25%, var(--panel-hover) 50%, var(--panel-inset) 75%);
  background-size: 200% 100%;
  animation: sk-shimmer 1.2s ease-in-out infinite;
}
.sk-title {
  width: 42%;
  height: 18px;
}
.sk-line {
  width: 100%;
}
.sk-line.short {
  width: 70%;
}
.sk-block {
  height: 64px;
}
.sk-block.short {
  width: 88%;
  height: 40px;
}
@keyframes sk-shimmer {
  0% {
    background-position: 200% 0;
  }
  100% {
    background-position: -200% 0;
  }
}
.detail-error {
  margin: 22px 0 0;
  padding: 10px 12px;
  font-size: 12px;
  color: var(--text-faint);
  background: var(--panel-inset);
  border-radius: var(--radius);
}

/* 笔记 */
.note-panel {
  flex: none;
  border-top: 1px solid var(--border);
  background: var(--panel-inset);
  padding: 10px 16px 12px;
}
.note-head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  margin-bottom: 6px;
}
.note-title {
  font-size: 12px;
  font-weight: 600;
}
.note-saved {
  font-size: 10.5px;
  color: var(--text-faint);
}
.note-area {
  width: 100%;
  resize: vertical;
  min-height: 64px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--panel);
  color: var(--text);
  font-family: inherit;
  font-size: 12.5px;
  line-height: 1.7;
  padding: 8px 10px;
  outline: none;
  transition: border-color 0.12s ease, box-shadow 0.12s ease;
}
.note-area:focus {
  border-color: color-mix(in srgb, var(--accent) 50%, var(--border));
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 14%, transparent);
}
.note-area::placeholder {
  color: var(--text-faint);
}
</style>
