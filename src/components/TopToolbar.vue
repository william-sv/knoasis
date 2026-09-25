<script setup>
// 顶栏：Logo（回浏览态·全部学科）· Home 入口 · 全局搜索（防抖+IME 兼容）· 主题 · 菜单
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useUi } from "../stores/ui.js";
import { useSearch } from "../stores/search.js";
import { useFavorites } from "../stores/favorites.js";
import { useNotes } from "../stores/notes.js";
import { downloadJson } from "../lib/format.js";
import logoAsset from "../assets/logo.png";
import homeAsset from "../assets/icons/home.png";
import darkAsset from "../assets/icons/dark.png";
import lightAsset from "../assets/icons/light.png";
import menuAsset from "../assets/icons/menu.png";
import { shouldFocusSearch } from "../lib/shortcuts.js";

const ui = useUi();
const search = useSearch();
const favorites = useFavorites();
const notes = useNotes();

// ---------- 搜索（debounce ~80ms + IME composition 兼容） ----------
const composing = ref(false);
let debounceTimer = null;

// 搜索输入框引用：供全局 ⌘K / Ctrl+K 聚焦并全选
const searchInput = ref(null);

// v-model 直接绑定 store.q，便于其它操作（切学科等）清空输入
const qModel = computed({
  get: () => search.q,
  set: (v) => {
    search.q = v;
  },
});

function scheduleSearch(delay = 80) {
  clearTimeout(debounceTimer);
  if (composing.value) return;
  debounceTimer = setTimeout(() => {
    search.run();
  }, delay);
}

watch(
  () => search.q,
  () => {
    // 组合态期间的中间变化不触发；组合结束后由 compositionend 统一触发一次
    if (!composing.value) scheduleSearch();
  },
);

function onSearchInput() {
  if (!composing.value) scheduleSearch();
}

function onCompositionStart() {
  composing.value = true;
  clearTimeout(debounceTimer);
}
function onCompositionEnd() {
  composing.value = false;
  // 组合结束立即搜一次，避免最后一个字因防抖被拖慢
  scheduleSearch(0);
}
function clearSearch() {
  search.clear();
}
function onSearchKeydown(e) {
  if (e.key === "Escape") {
    clearSearch();
    e.target.blur();
  }
}

// ---------- 导航：Logo 回浏览态总览 / Home 入口 ----------
const menuOpen = ref(false);

function onLogoClick() {
  menuOpen.value = false;
  ui.gotoBrowse();
  if (ui.activeSetId !== "all") ui.gotoAll();
}

function onHomeClick() {
  menuOpen.value = false;
  ui.gotoHome();
}

// ---------- 主题：浅色显示深色图标（点击转深色），深色显示浅色图标 ----------
const resolvedTheme = computed(() => ui.resolvedTheme);
const themeIcon = computed(() => (resolvedTheme.value === "dark" ? lightAsset : darkAsset));

function onThemeClick() {
  ui.toggleTheme();
}

const themeBtnLabel = computed(() =>
  resolvedTheme.value === "dark" ? "切换到浅色模式" : "切换到深色模式",
);

// ---------- 菜单 ----------
function exportData() {
  const payload = {
    app: "Knoasis",
    kind: "user-data-export",
    version: "0.1.0-ui-prototype",
    exportedAt: new Date().toISOString(),
    favorites: favorites.items.map((i) => ({ ...i })),
    notes: Object.keys(notes.map).map((uid) => ({
      uid,
      content: notes.map[uid].content,
      updatedAt: notes.map[uid].updatedAt,
    })),
  };
  const stamp = new Date().toISOString().slice(0, 19).replace(/[:T]/g, "-");
  downloadJson(`knoasis-userdata-${stamp}.json`, payload);
  menuOpen.value = false;
  ui.showToast("已导出收藏与笔记 JSON");
}

function openSetsManager() {
  menuOpen.value = false;
  ui.openSetsManager();
}

function openFavorites() {
  menuOpen.value = false;
  ui.openPanel("favorites");
}

function stopProp(e) {
  e.stopPropagation();
}

// ---------- 全局快捷键：Cmd/Ctrl + K 聚焦搜索框 ----------
// 仅非输入态抢占（焦点已在输入类元素内时不处理）；命中时 preventDefault 阻止默认行为。
function onGlobalKeydown(e) {
  if (!shouldFocusSearch(e, document.activeElement)) return;
  e.preventDefault();
  const el = searchInput.value;
  if (!el) return;
  el.focus();
  // 已有文本则全选，便于直接输入覆盖
  if (typeof el.select === "function") el.select();
}

onMounted(() => {
  window.addEventListener("keydown", onGlobalKeydown);
});

// 卸载时清理定时器与全局监听
onBeforeUnmount(() => {
  clearTimeout(debounceTimer);
  window.removeEventListener("keydown", onGlobalKeydown);
});
</script>

<template>
  <header class="toolbar">
    <!-- Logo：点击回浏览态总览 -->
    <button class="brand" title="回到浏览" @click="onLogoClick">
      <img class="brand-logo" :src="logoAsset" alt="诺西斯" />
      <span class="brand-name">Knoasis</span>
      <span class="brand-sub">诺西斯</span>
    </button>

    <!-- Home 入口（原学科下拉位置）：切到首页整页 -->
    <button
      class="home-btn"
      :class="{ 'is-active': ui.view === 'home' }"
      title="首页"
      @click="onHomeClick"
    >
      <img class="home-icon" :src="homeAsset" alt="首页" />
    </button>

    <!-- 全局搜索框 -->
    <div class="search">
      <span class="search-icon"><svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="7"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg></span>
      <input
        ref="searchInput"
        v-model="qModel"
        class="search-input"
        type="text"
        placeholder="搜索知识点：名称 / 别名 / 关键词"
        spellcheck="false"
        autocomplete="off"
        @input="onSearchInput"
        @compositionstart="onCompositionStart"
        @compositionend="onCompositionEnd"
        @keydown="onSearchKeydown"
      />
      <button v-if="search.q" class="search-clear" title="清除搜索" @click="clearSearch">
        ✕
      </button>
      <span v-else class="search-kbd">⌘K</span>
    </div>

    <!-- 右侧图标区 -->
    <div class="spacer"></div>
    <div class="actions">
      <!-- 主题切换 -->
      <button class="icon-btn" :title="themeBtnLabel" @click="onThemeClick">
        <img class="theme-icon" :src="themeIcon" alt="" />
      </button>

      <!-- 菜单：管理学科包 / 收藏夹 / 导出收藏夹 -->
      <div class="action-wrap">
        <button
          class="icon-btn"
          title="菜单"
          :class="{ 'icon-btn--on': menuOpen }"
          @click="menuOpen = !menuOpen"
        >
          <img class="menu-icon" :src="menuAsset" alt="菜单" />
        </button>
        <template v-if="menuOpen">
          <div class="pop-scrim" @click="menuOpen = false"></div>
          <div class="pop pop--right" @click="stopProp">
            <button class="pop-item" @click="openSetsManager">
              <span>管理学科包</span>
            </button>
            <div class="pop-sep"></div>
            <button class="pop-item" @click="openFavorites">
              <span>收藏夹</span>
            </button>
            <div class="pop-sep"></div>
            <button class="pop-item" @click="exportData">
              <span>导出收藏夹</span>
            </button>
          </div>
        </template>
      </div>
    </div>
  </header>
</template>

<style scoped>
.toolbar {
  display: flex;
  align-items: center;
  gap: 10px;
  height: var(--toolbar-h);
  padding: 0 14px;
  background: var(--panel);
  border-bottom: 1px solid var(--border);
  flex: none;
  z-index: 40;
}

/* 品牌 */
.brand {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  padding: 4px 8px 4px 2px;
  border-radius: var(--radius-sm);
  white-space: nowrap;
}
.brand-logo {
  height: 22px;
  width: auto;
  flex: none;
}
.brand:hover {
  background: var(--panel-hover);
}
.brand-name {
  font-size: 15px;
  font-weight: 700;
  letter-spacing: 0.2px;
  color: var(--text);
}
.brand-sub {
  font-size: 11px;
  color: var(--text-faint);
  letter-spacing: 1px;
}

/* Home 入口 */
.home-btn {
  flex: none;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 30px;
  height: 30px;
  border-radius: var(--radius-sm);
  transition: background 0.12s ease;
}
.home-btn:hover {
  background: var(--panel-hover);
}
.home-btn.is-active {
  background: rgba(var(--accent-rgb), 0.12);
}
.home-icon {
  width: 18px;
  height: 18px;
  object-fit: contain;
  display: block;
}

/* 搜索框 */
.search {
  position: relative;
  display: flex;
  align-items: center;
  flex: 1;
  max-width: 560px;
  height: 30px;
  margin-left: 60px;
  background: var(--field-bg);
  border: 1px solid transparent;
  border-radius: var(--radius-sm);
  transition: background 0.12s ease, border-color 0.12s ease, box-shadow 0.12s ease;
}
.search:focus-within {
  background: var(--panel);
  border-color: rgba(var(--accent-rgb), 0.5);
  box-shadow: 0 0 0 3px rgba(var(--accent-rgb), 0.14);
}
.search-icon {
  font-size: 12px;
  opacity: 0.6;
  margin-left: 9px;
  flex: none;
}
.search-input {
  flex: 1;
  min-width: 0;
  border: none;
  outline: none;
  background: transparent;
  font-family: inherit;
  font-size: 12.5px;
  color: var(--text);
  padding: 0 8px;
}
.search-input::placeholder {
  color: var(--text-faint);
}
.search-clear {
  width: 22px;
  height: 22px;
  margin-right: 4px;
  border-radius: 50%;
  font-size: 11px;
  color: var(--text-muted);
  background: rgba(var(--text-muted-rgb), 0.16);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex: none;
}
.search-clear:hover {
  background: rgba(var(--text-muted-rgb), 0.3);
  color: var(--text);
}
.search-kbd {
  margin-right: 8px;
  padding: 1px 5px;
  border: 1px solid var(--border);
  border-radius: 4px;
  font-size: 10px;
  color: var(--text-faint);
  background: var(--panel);
  flex: none;
}

.spacer {
  flex: 1;
}

/* 右侧图标 */
.actions {
  display: flex;
  align-items: center;
  gap: 2px;
  flex: none;
}
.action-wrap {
  position: relative;
}
.theme-icon,
.menu-icon {
  width: 18px;
  height: 18px;
  object-fit: contain;
  display: block;
}
</style>
