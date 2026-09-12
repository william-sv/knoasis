// Pinia store：useSearch
// 职责：搜索词 / 命中结果 / loading 态（v1.3 §6 形状）+ 属性筛选（等级/类型/分类）。
// IME 组合态在 TopToolbar 中处理（compositionstart/end 期间不触发 run），
// 防抖 ~80ms 也在组件层完成；本 store 负责一次“真正执行”。
//
// 筛选（filter）与搜索（query）正交、可共存：两者在知识集 store 的 baseList 里以 AND 组合。
// 筛选由详情页标签触发（EntryDetail 的 chip），不是工具栏下拉，避免恢复已下线的下拉式筛选 UI。

import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { searchEntries } from "../lib/search.js";
import { useKnowledgeSets } from "./knowledgeSets.js";
import { useUi } from "./ui.js";

/** 筛选维度：等级（level code）/ 类型（type）/ 分类（category）；null = 未启用 */
function emptyFilters() {
  return { level: null, type: null, category: null };
}

export const useSearch = defineStore("search", () => {
  // ---- state ----
  const q = ref(""); // 原始输入（含未提交 IME 中间态；执行时以非组合态为准）
  const hits = ref([]); // 命中条目数组
  const loading = ref(false);
  const tookMs = ref(0);
  const route = ref(""); // 最近一次搜索范围（setId | all），供状态显示
  const filters = ref(emptyFilters()); // 属性筛选（详情页标签触发）

  // ---- getters ----
  const active = computed(() => q.value.trim().length > 0);
  const filterActive = computed(
    () =>
      filters.value.level != null ||
      filters.value.type != null ||
      filters.value.category != null,
  );

  // 执行搜索（同步，条目量小；将来替换为带 reqId 的 IPC 调用）
  function run() {
    const ui = useUi();
    const ks = useKnowledgeSets();
    const query = q.value.trim();
    if (!query) {
      hits.value = [];
      loading.value = false;
      tookMs.value = 0;
      route.value = "";
      return;
    }
    loading.value = true;
    const t0 = performance.now();
    const scope =
      ui.activeSetId === "all" ? ks.entries : ks.entries.filter((e) => e.discipline === ui.activeSetId);
    hits.value = searchEntries(scope, query);
    tookMs.value = Math.round(performance.now() - t0);
    route.value = ui.activeSetId;
    loading.value = false;
  }

  function clear() {
    q.value = "";
    hits.value = [];
    loading.value = false;
    tookMs.value = 0;
    route.value = "";
  }

  /**
   * 设置 / 切换某维度筛选值（同值再次点击 → 取消该筛选，即 toggle）。
   * @param {'level'|'type'|'category'} key
   * @param {string} value 目标值（等级为 level code，含 '' 表示「未分级」）
   */
  function setFilter(key, value) {
    if (!(key in filters.value)) return;
    const next = { ...filters.value };
    next[key] = next[key] === value ? null : value;
    filters.value = next;
  }

  /** 清除全部筛选（不影响搜索关键词） */
  function clearFilters() {
    if (!filterActive.value) return;
    filters.value = emptyFilters();
  }

  return {
    q,
    hits,
    loading,
    tookMs,
    route,
    filters,
    active,
    filterActive,
    run,
    clear,
    setFilter,
    clearFilters,
  };
});
