<script setup>
// 收藏面板：右滑抽屉，列出收藏条目（跨学科），可点击跳转 / 取消收藏
import { useUi } from "../stores/ui.js";
import { useFavorites } from "../stores/favorites.js";
import { useKnowledgeSets } from "../stores/knowledgeSets.js";
import { typeLabel } from "../lib/format.js";

const ui = useUi();
const favorites = useFavorites();
const ks = useKnowledgeSets();

function openEntry(item) {
  const obj = ks.entryByUid.get(item.uid) ?? null;
  if (!obj) {
    // 悬空收藏：db 被替换后旧收藏仍保留，点击提示（不改面板结构）
    ui.showToast("条目已不在当前数据中");
    return;
  }
  ui.jumpToEntry(obj);
  ui.closePanel();
}

function removeItem(uid, e) {
  e.stopPropagation();
  favorites.remove(uid);
  ui.showToast("已取消收藏");
}
</script>

<template>
  <aside class="fav-panel">
    <header class="panel-head">
      <div class="panel-title">收藏 <span class="panel-count">{{ favorites.count }}</span></div>
      <button class="icon-btn" title="关闭" @click="ui.closePanel">✕</button>
    </header>

    <div v-if="favorites.list.length" class="panel-list">
      <button
        v-for="item in favorites.list"
        :key="item.uid"
        class="fav-item"
        :class="{ 'is-current': ui.selectedUid === item.uid }"
        @click="openEntry(item)"
      >
        <div class="fav-main">
          <div class="fav-name-row">
            <span class="dot" :style="{ background: item.set ? item.set.color : 'var(--text-faint)' }"></span>
            <span class="fav-name">{{ item.name }}</span>
          </div>
          <p class="fav-summary">{{ item.summary }}</p>
          <div class="fav-meta">
            <span class="chip">{{ item.set ? item.set.name : item.discipline }}</span>
            <span class="chip">{{ typeLabel(item.type) }}</span>
          </div>
        </div>
        <span class="fav-remove" title="取消收藏" @click.stop="removeItem(item.uid, $event)">✕</span>
      </button>
    </div>

    <!-- 空态 -->
    <div v-else class="empty panel-empty">
      <div class="empty-title">还没有收藏</div>
      <div class="empty-hint">在详情底部点击「收藏」，常用知识点会出现在这里，跨学科统一管理。</div>
    </div>
  </aside>
</template>

<style scoped>
.fav-panel {
  position: absolute;
  top: 0;
  right: 0;
  bottom: 0;
  width: 340px;
  max-width: 86%;
  display: flex;
  flex-direction: column;
  background: var(--panel);
  border-left: 1px solid var(--border);
  box-shadow: var(--shadow-2);
  z-index: 90;
}

.panel-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 48px;
  padding: 0 8px 0 18px;
  border-bottom: 1px solid var(--border-soft);
  flex: none;
}
.panel-title {
  font-size: 14px;
  font-weight: 650;
}
.panel-count {
  display: inline-block;
  min-width: 20px;
  height: 18px;
  padding: 0 6px;
  margin-left: 4px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--accent) 12%, var(--panel));
  color: var(--accent);
  font-size: 11px;
  line-height: 18px;
  text-align: center;
}

.panel-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 10px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.fav-item {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  width: 100%;
  padding: 10px 10px 10px 12px;
  border-radius: var(--radius);
  text-align: left;
  border: 1px solid transparent;
  transition: background 0.12s ease, border-color 0.12s ease;
}
.fav-item:hover {
  background: var(--panel-hover);
}
.fav-item.is-current {
  background: color-mix(in srgb, var(--accent) 6%, var(--panel));
  border-color: color-mix(in srgb, var(--accent) 22%, var(--border));
}
.fav-main {
  flex: 1;
  min-width: 0;
}
.fav-name-row {
  display: flex;
  align-items: center;
  gap: 7px;
}
.dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex: none;
}
.fav-name {
  font-size: 13px;
  font-weight: 600;
  color: var(--text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.fav-item.is-current .fav-name {
  color: var(--accent);
}
.fav-summary {
  margin: 4px 0 7px;
  font-size: 11.5px;
  color: var(--text-muted);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
.fav-meta {
  display: flex;
  gap: 6px;
}
.chip {
  height: 17px;
  padding: 0 6px;
  font-size: 10px;
  line-height: 17px;
}
.fav-remove {
  width: 18px;
  height: 18px;
  flex: none;
  border-radius: 50%;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 10px;
  color: var(--text-faint);
  opacity: 0;
  transition: opacity 0.12s ease, background 0.12s ease;
}
.fav-item:hover .fav-remove {
  opacity: 1;
}
.fav-remove:hover {
  background: color-mix(in srgb, #e5484d 14%, transparent);
  color: #e5484d;
}

.panel-empty {
  padding: 30px;
}
</style>
