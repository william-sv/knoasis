<script setup>
// 左列：知识条目列表（按 category 分组的「灰色副标题 + 标题」）
// 列表只展示标题；同一 category 归为一组，组头以灰色小字呈现（如数学「平面几何」、英语「时态」）。
import { computed } from "vue";
import { useKnowledgeSets } from "../stores/knowledgeSets.js";
import { useUi } from "../stores/ui.js";
import { useSearch } from "../stores/search.js";
import { useFavorites } from "../stores/favorites.js";

const ks = useKnowledgeSets();
const ui = useUi();
const search = useSearch();
const favorites = useFavorites();

const list = computed(() => ks.visibleEntries);

// 按 category 分组：灰色副标题（如「平面几何」）+ 其下标题列表
const groups = computed(() => {
  const map = new Map();
  for (const item of list.value) {
    const key = item.category || "未分类";
    if (!map.has(key)) map.set(key, []);
    map.get(key).push(item);
  }
  return [...map.entries()].map(([label, items]) => ({ label, items }));
});

// 空态上下文：搜索无结果 / 数据未加载（浏览器预览）/ 真实数据加载失败 / 无学科 / 学科无条目
const emptyState = computed(() => {
  if (search.active) {
    return {
      title: "没有找到相关知识点",
      hint: `没有与“${search.q.trim()}”匹配的结果。试试名称、别名或更短的关键词。`,
      cta: "清除搜索",
      onCta: () => search.clear(),
    };
  }
  if (ks.loading) {
    return {
      title: "正在加载数据…",
      hint: "正在从本地学科包读取真实数据。",
    };
  }
  if (ks.source === "error") {
    return {
      title: "真实数据加载失败",
      hint: `${ks.loadError || "未知错误"}。请检查学科包后重试。`,
      cta: "重新加载",
      onCta: async () => {
        await ks.reload();
      },
    };
  }
  if (ks.source === "empty") {
    return {
      title: "浏览器预览 · 未加载数据",
      hint: "请在桌面应用（Tauri）中打开 Knoasis，以加载学科包真实数据。",
    };
  }
  if (ui.activeSetId === "all") {
    return {
      title: "还没有知识集",
      hint: "安装一个知识集后，这里会显示可浏览的知识点。",
    };
  }
  const s = ks.setById.get(ui.activeSetId);
  return {
    title: `「${s ? s.name : ""}」暂无知识点`,
    hint: "这个知识集还没有可浏览的条目。",
  };
});

function select(item) {
  if (ui.selectedUid !== item.uid) ui.selectEntry(item.uid);
}
</script>

<template>
  <div class="entry-list" role="listbox">
    <!-- 分组列表 -->
    <div v-if="list.length" class="list-inner">
      <template v-for="group in groups" :key="group.label">
        <div class="group-label">{{ group.label }}</div>
        <button
          v-for="item in group.items"
          :key="item.uid"
          class="card"
          :class="{ 'is-selected': ui.selectedUid === item.uid }"
          role="option"
          :aria-selected="ui.selectedUid === item.uid"
          @click="select(item)"
        >
          <span class="card-title">{{ item.name }}</span>
          <span
            v-if="favorites.isFavorite(item.uid)"
            class="fav-dot"
            title="已收藏"
          ></span>
        </button>
      </template>
    </div>

    <!-- 空态 -->
    <div v-else class="empty">
      <div class="empty-title">{{ emptyState.title }}</div>
      <div class="empty-hint">{{ emptyState.hint }}</div>
      <button v-if="emptyState.cta" class="btn btn--primary" @click="emptyState.onCta && emptyState.onCta()">
        {{ emptyState.cta }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.entry-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 8px 8px 12px;
  /* 列表文本可选中、可 Cmd+C 复制 */
  user-select: text;
  -webkit-user-select: text;
}

.list-inner {
  display: flex;
  flex-direction: column;
  gap: 1px;
}

/* 分组灰色副标题 */
.group-label {
  margin: 12px 6px 4px;
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.3px;
  color: var(--text-faint);
}
.group-label:first-child {
  margin-top: 4px;
}

.card {
  display: flex;
  align-items: center;
  width: 100%;
  text-align: left;
  border-radius: var(--radius-sm);
  border: none;
  background: transparent;
  position: relative;
  transition: background 0.12s ease;
}
.card:hover {
  background: var(--panel-hover);
}
.card.is-selected {
  background: color-mix(in srgb, var(--accent) 9%, var(--panel));
}
/* 选中：左侧竖条 */
.card::before {
  content: "";
  position: absolute;
  left: 0;
  top: 6px;
  bottom: 6px;
  width: 3px;
  border-radius: 0 3px 3px 0;
  background: var(--accent);
  opacity: 0;
  transition: opacity 0.12s ease;
}
.card.is-selected::before {
  opacity: 1;
}

.card-title {
  flex: 1;
  min-width: 0;
  padding: 7px 10px 7px 12px;
  font-size: 13px;
  font-weight: 550;
  color: var(--text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.card.is-selected .card-title {
  color: var(--accent);
}

.fav-dot {
  width: 6px;
  height: 6px;
  margin-right: 10px;
  border-radius: 50%;
  background: var(--accent);
  flex: none;
}

.empty {
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 24px;
  text-align: center;
}
.empty-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-muted);
}
.empty-hint {
  font-size: 12px;
  line-height: 1.6;
  color: var(--text-faint);
  max-width: 240px;
}
</style>
