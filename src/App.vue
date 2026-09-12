<script setup>
// Knoasis UI 原型 · 根布局
// 顶栏 + 双栏（左列表 / 右详情，可拖分隔条）+ 底部状态栏
// 承载全局层：收藏面板、搜索空态灰遮罩、toast；并驱动 data-theme 深色切换。
import { onBeforeUnmount, ref, watch } from "vue";
import { useUi } from "./stores/ui.js";
import { useKnowledgeSets } from "./stores/knowledgeSets.js";
import { useSearch } from "./stores/search.js";
import TopToolbar from "./components/TopToolbar.vue";
import ListToolbar from "./components/ListToolbar.vue";
import EntryList from "./components/EntryList.vue";
import EntryDetail from "./components/EntryDetail.vue";
import FavoritesPanel from "./components/FavoritesPanel.vue";
import SetManagerDialog from "./components/SetManagerDialog.vue";
import DownloadDialog from "./components/DownloadDialog.vue";
import HomeView from "./components/HomeView.vue";
import SettingsView from "./components/SettingsView.vue";
import StatusBar from "./components/StatusBar.vue";

const ui = useUi();
const ks = useKnowledgeSets();
const search = useSearch();

// ---- 主题：resolvedTheme → <html data-theme> ----
watch(
  () => ui.resolvedTheme,
  (t) => ui.applyTheme(t),
  { immediate: true },
);

// ---- 启动：store 初始化在 useKnowledgeSets() 时自动触发（Tauri 异步 IPC 加载真实数据；浏览器为空态） ----
// 数据就绪后再决定落地视图：URL hash 深链优先（dev / 分享链接）；否则按记忆的学科包路由
// （无记忆 → 首页；记忆有效 → 对应学科浏览页；记忆失效应已由 applyStartupRoute 清掉并回落首页）。
watch(
  () => ks.loaded,
  (v) => {
    if (!v) return;
    const deepLinked = ui.loadFromHash();
    if (!deepLinked) ui.applyStartupRoute();
    if (ui.view === "browse" && !ui.selectedUid && ks.visibleEntries.length) {
      ui.selectDefault();
    }
  },
  { immediate: true },
);

// 筛选 / 学科范围变化后：若当前选中条目已不在可见列表中 → 清空选中（详情回到空态）。
// 搜索态例外：沿用「保留上次详情 + 灰遮罩」的既有体验，交给搜索空态处理。
watch(
  () => ks.visibleEntries,
  (list) => {
    if (search.active) return;
    if (ui.selectedUid && !list.some((e) => e.uid === ui.selectedUid)) {
      ui.clearSelection();
    }
  },
);

// 首次真实数据加载失败（source → error）→ toast；列表区空态 / 状态栏已承载持久提示
watch(
  () => ks.source,
  (now, prev) => {
    if (now === "error" && prev !== "error" && prev !== "ipc") {
      ui.showToast("学科包加载失败：请在「管理学科包」中检查或导入学科包", 2800);
    }
  },
);

// 搜索空态（保留上次详情 + 灰遮罩）：有搜索词且 0 命中
const showSearchBlanket = ref(false);
watch(
  () => [search.active, search.hits.length],
  () => {
    showSearchBlanket.value = search.active && search.hits.length === 0;
  },
  { immediate: true },
);

function clearSearchForBlanket() {
  search.clear();
}

// ---- 拖拽分隔条（左列 25%–50%） ----
const appRef = ref(null);
const dragging = ref(false);

function onDividerDown(e) {
  if (e.button !== 0) return;
  e.preventDefault();
  dragging.value = true;
  window.addEventListener("pointermove", onDividerMove);
  window.addEventListener("pointerup", onDividerUp);
  window.addEventListener("pointercancel", onDividerUp);
}

function onDividerMove(e) {
  const rect = appRef.value.getBoundingClientRect();
  if (!rect.width) return;
  const pct = ((e.clientX - rect.left) / rect.width) * 100;
  ui.setSplit(pct);
}

function onDividerUp() {
  dragging.value = false;
  ui.persistSplit();
  window.removeEventListener("pointermove", onDividerMove);
  window.removeEventListener("pointerup", onDividerUp);
  window.removeEventListener("pointercancel", onDividerUp);
}

onBeforeUnmount(() => {
  window.removeEventListener("pointermove", onDividerMove);
  window.removeEventListener("pointerup", onDividerUp);
  window.removeEventListener("pointercancel", onDividerUp);
});
</script>

<template>
  <div
    ref="appRef"
    class="app-shell"
    :class="{ 'is-dragging': dragging }"
    :style="{ '--accent': ui.accentColor }"
  >
    <TopToolbar />

    <!-- 主工作区：按 ui.view 切换（browse=双栏 / home=首页 / settings=设置） -->
    <main class="workspace">
      <!-- 浏览态：左列表 + 分隔条 + 右详情 -->
      <template v-if="ui.view === 'browse'">
        <!-- 左列：工具条 + 列表 -->
        <section
          class="pane pane-left"
          :style="{ width: ui.split + '%', flex: `0 0 ${ui.split}%` }"
        >
          <ListToolbar />
          <EntryList />
        </section>

        <!-- 拖拽分隔条 -->
        <div
          class="divider"
          role="separator"
          aria-orientation="vertical"
          title="拖动调整左右宽度"
          @pointerdown="onDividerDown"
        >
          <span class="divider-grip"></span>
        </div>

        <!-- 右列：详情 -->
        <section class="pane pane-right">
          <EntryDetail />

          <!-- 搜索无结果：保留上次详情 + 灰遮罩 -->
          <Transition name="fade">
            <div v-if="showSearchBlanket" class="search-blanket">
              <div class="blanket-card">
                <div class="blanket-icon"><svg viewBox="0 0 24 24" width="30" height="30" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="7"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg></div>
                <div class="blanket-title">没有找到「{{ search.q }}」</div>
                <div class="blanket-hint">
                  搜索范围为“全部/当前学科”的名称、别名与摘要。<br />
                  可尝试更短的关键词，或清除搜索继续浏览。
                </div>
                <button class="btn btn--primary" @click="clearSearchForBlanket">
                  清除搜索 · 查看全部 {{ ks.totalCount }} 条
                </button>
              </div>
            </div>
          </Transition>
        </section>
      </template>

      <!-- 首页整页 -->
      <HomeView v-else-if="ui.view === 'home'" />

      <!-- 设置整页 -->
      <SettingsView v-else />
    </main>

    <StatusBar />

    <!-- 全局层：收藏面板 + 遮罩 -->
    <Transition name="fade">
      <div v-if="ui.panel" class="scrim" @click="ui.closePanel"></div>
    </Transition>
    <Transition name="slide">
      <FavoritesPanel v-if="ui.panel === 'favorites'" />
    </Transition>

    <!-- 学科包管理（导入 / 删除 / 启用停用） -->
    <Transition name="fade">
      <SetManagerDialog v-if="ui.setsManagerOpen" />
    </Transition>

    <!-- 学科包下载（演示数据） -->
    <Transition name="fade">
      <DownloadDialog v-if="ui.downloadOpen" />
    </Transition>

    <!-- Toast -->
    <Transition name="toast">
      <div v-if="ui.toast" class="toast" :key="ui.toast.id">{{ ui.toast.message }}</div>
    </Transition>
  </div>
</template>

<style scoped>
.app-shell {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--bg);
  position: relative;
  overflow: hidden;
}
.app-shell.is-dragging {
  cursor: col-resize;
}
.app-shell.is-dragging * {
  user-select: none;
}

.workspace {
  display: flex;
  flex: 1;
  min-height: 0;
  position: relative;
}

.pane {
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
}
.pane-left {
  position: relative;
  background: var(--bg);
}
.pane-right {
  flex: 1 1 auto;
  position: relative;
  background: var(--panel);
}

/* 分隔条 */
.divider {
  flex: none;
  width: 5px;
  margin: 0 -2px;
  cursor: col-resize;
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 5;
  position: relative;
}
.divider::after {
  content: "";
  position: absolute;
  inset: 0;
}
.divider-grip {
  width: 2px;
  height: 44px;
  border-radius: 2px;
  background: var(--border);
  transition: background 0.12s ease;
}
.divider:hover .divider-grip,
.app-shell.is-dragging .divider-grip {
  background: var(--accent);
}

/* 搜索空态灰遮罩 */
.search-blanket {
  position: absolute;
  inset: 0;
  z-index: 30;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--blanket);
  backdrop-filter: blur(1.5px);
}
.blanket-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  max-width: 340px;
  text-align: center;
  padding: 26px 28px;
  border-radius: 12px;
  background: var(--panel);
  border: 1px solid var(--border);
  box-shadow: var(--shadow-2);
}
.blanket-icon {
  font-size: 26px;
  line-height: 1;
}
.blanket-title {
  font-size: 14px;
  font-weight: 650;
  color: var(--text);
  word-break: break-all;
}
.blanket-hint {
  font-size: 12px;
  color: var(--text-muted);
  line-height: 1.7;
}

/* 收藏面板遮罩 */
.scrim {
  position: absolute;
  inset: 0;
  z-index: 80;
  background: var(--scrim);
}

/* Toast */
.toast {
  position: absolute;
  left: 50%;
  bottom: calc(var(--status-h) + 18px);
  transform: translateX(-50%);
  z-index: 200;
  padding: 8px 16px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--text) 88%, transparent);
  color: var(--bg);
  font-size: 12px;
  box-shadow: var(--shadow-2);
  pointer-events: none;
  white-space: nowrap;
}

/* 过渡 */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.16s ease;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

.slide-enter-active,
.slide-leave-active {
  transition: transform 0.22s ease;
}
.slide-enter-from,
.slide-leave-to {
  transform: translateX(100%);
}

.toast-enter-active,
.toast-leave-active {
  transition: opacity 0.2s ease, transform 0.2s ease;
}
.toast-enter-from,
.toast-leave-to {
  opacity: 0;
  transform: translateX(-50%) translateY(6px);
}
</style>
