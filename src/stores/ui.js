// Pinia store：useUi
// 职责：当前视图状态 —— 学科/全部、选中条目、主题、筛选、面板、拖拽宽度、toast。
// v1.3 §6 的 useSettings + useViewer 的轻量部分在本原型中并入此处。

import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { useKnowledgeSets } from "./knowledgeSets.js";
import { useSearch } from "./search.js";
import {
  clearActiveSetPref,
  readActiveSetPref,
  resolveStartupView,
  writeActiveSetPref,
} from "../lib/startup.js";

const LS_THEME = "knoasis.theme.v1";
const LS_SPLIT = "knoasis.split.v2";

const DEFAULT_ACCENT = "#2f9e77";

function readSystemTheme() {
  if (typeof window === "undefined" || !window.matchMedia) return "light";
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

function loadThemePref() {
  try {
    const v = localStorage.getItem(LS_THEME);
    return v === "light" || v === "dark" ? v : "system";
  } catch {
    return "system";
  }
}

function loadSplit() {
  try {
    const v = Number(localStorage.getItem(LS_SPLIT));
    return Number.isFinite(v) && v >= 16 && v <= 50 ? v : 24;
  } catch {
    return 35;
  }
}

export const useUi = defineStore("ui", () => {
  const knowledgeSets = useKnowledgeSets();
  const search = useSearch();

  // ---- 视图 ----
  // 启动初值：有记忆则乐观进入浏览页（数据就绪后再由 applyStartupRoute 校验，
  // 若记忆的学科包已被删除/停用则回落首页），无记忆则停在首页（首次启动引导）。
  const initialActiveSet = readActiveSetPref();
  const view = ref(initialActiveSet ? "browse" : "home"); // 'browse'（双栏工作区）| 'home'（首页整页）| 'settings'（设置整页）
  const activeSetId = ref("all"); // 'all' | 学科 set.id（真实数据由 IPC 注册）
  const selectedUid = ref(null);
  const split = ref(loadSplit()); // 左列宽度 %

  // ---- 主题 ----
  const themePref = ref(loadThemePref()); // system | light | dark
  const systemTheme = ref(readSystemTheme());
  const resolvedTheme = computed(() =>
    themePref.value === "system" ? systemTheme.value : themePref.value,
  );

  // ---- 全局层 ----
  const panel = ref(null); // null | 'favorites'
  const toast = ref(null); // { id, message }
  const setsManagerOpen = ref(false); // 学科包管理弹窗（导入/删除/启用停用）
  const downloadOpen = ref(false); // 学科包下载弹窗（演示数据）

  // 监听系统主题变化（仅在跟随系统时生效）
  let mql = null;
  function bindSystemTheme() {
    if (typeof window === "undefined" || !window.matchMedia) return;
    mql = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = (e) => {
      systemTheme.value = e.matches ? "dark" : "light";
    };
    if (mql.addEventListener) mql.addEventListener("change", onChange);
    else if (mql.addListener) mql.addListener(onChange);
  }
  bindSystemTheme();

  // ---- getters ----
  const activeSet = computed(() => {
    if (activeSetId.value === "all") return null;
    return knowledgeSets.setById.get(activeSetId.value) ?? null;
  });

  // 强调色随学科：全部态用品牌绿
  const accentColor = computed(() =>
    activeSet.value ? activeSet.value.color : DEFAULT_ACCENT,
  );

  const selectedEntry = computed(() => {
    if (!selectedUid.value) return null;
    return knowledgeSets.entryByUid.get(selectedUid.value) ?? null;
  });

  // 当前可视条目首条（用于切换学科后自动选中）
  // 注意：跨 store 访问时 Pinia 会自动 unwrap 计算属性，无需再 .value。
  const firstVisible = computed(() => knowledgeSets.visibleEntries[0] ?? null);

  // ---- actions ----
  function applyTheme(theme) {
    const t = theme === "system" ? readSystemTheme() : theme;
    document.documentElement.setAttribute("data-theme", t);
  }

  // 切换到某学科（或全部）：清理搜索与筛选，记忆选择，并自动选中首条
  function switchSet(setId) {
    activeSetId.value = setId;
    search.clear();
    search.clearFilters();
    persistActiveSet(setId);
    selectedUid.value = null;
    selectDefault();
  }

  // 记忆当前学科选择（选择「全部」也算有效选择）
  function persistActiveSet(setId) {
    writeActiveSetPref(setId);
  }

  /**
   * 从详情页「学科」标签切换到该学科（toggle：再次点击同科回到全部）。
   * 与 switchSet 不同：保留当前选中条目（该条目属于目标学科，切换后仍在范围内可见），
   * 语义上「切换学科」比「筛选」更贴近点击学科标签的预期。
   */
  function selectDiscipline(discipline) {
    if (!discipline) return;
    const target = activeSetId.value === discipline ? "all" : discipline;
    activeSetId.value = target;
    search.clear();
    search.clearFilters();
    persistActiveSet(target);
    // 不重置 selectedUid：条目在新的学科范围内依然可见（失配时由 App 层兜底清空）
  }

  // 清空当前选中（详情页回到空态）
  function clearSelection() {
    selectedUid.value = null;
  }

  /**
   * 启动路由：等学科包列表加载完成后调用，依据记忆决定首页 / 浏览页。
   *
   * 失效记忆清理有严格前提：**真实数据确实加载成功**（source==='ipc'）且确实没有该包，
   * 才能判定「记忆已失效」并清键。若本轮是「加载失败」（source==='error'）或浏览器空态
   * （source==='empty'），sets 为空只是「暂时没读到」，并非包被删除——此时必须保留记忆，
   * 否则一次偶发加载失败就会把用户的有效选择误清掉，下次启动莫名回到首页。
   */
  function applyStartupRoute() {
    const res = resolveStartupView(readActiveSetPref(), knowledgeSets.sets);
    if (res.clearMemory && knowledgeSets.source === "ipc") clearActiveSetPref();
    activeSetId.value = res.activeSetId;
    view.value = res.view;
  }

  // 顶部 Logo：回到“全部学科”总览态
  function gotoAll() {
    switchSet("all");
  }

  // ---- 顶层视图切换（不改动 selectedUid，返回浏览态时保持原选中） ----
  function gotoHome() {
    view.value = "home";
  }
  function gotoSettings() {
    view.value = "settings";
  }
  function gotoBrowse() {
    view.value = "browse";
  }

  // 跳转到某条目：若目标属于其它学科，先切学科再定位
  function jumpToEntry(entry) {
    if (!entry) return;
    if (activeSetId.value !== entry.discipline) {
      activeSetId.value = entry.discipline;
      search.clear();
    }
    selectedUid.value = entry.uid;
  }

  function selectEntry(uid) {
    selectedUid.value = uid;
  }

  function selectDefault() {
    if (firstVisible.value) selectedUid.value = firstVisible.value.uid;
  }

  // ---- 主题切换（浅色/深色）：把“当前解析态”翻转并固化为用户偏好 ----
  function toggleTheme() {
    themePref.value = resolvedTheme.value === "dark" ? "light" : "dark";
    try {
      localStorage.setItem(LS_THEME, themePref.value);
    } catch {
      /* 隐私模式忽略 */
    }
    applyTheme(themePref.value);
  }
  function setThemePref(pref) {
    themePref.value = pref;
    try {
      localStorage.setItem(LS_THEME, pref);
    } catch {
      /* ignore */
    }
    applyTheme(pref);
  }

  // ---- 面板 ----
  function openPanel(name) {
    panel.value = name;
  }
  function closePanel() {
    panel.value = null;
  }
  function togglePanel(name) {
    panel.value = panel.value === name ? null : name;
  }

  // ---- 学科包管理弹窗 ----
  function openSetsManager() {
    setsManagerOpen.value = true;
  }
  function closeSetsManager() {
    setsManagerOpen.value = false;
  }

  // ---- 学科包下载弹窗（演示数据） ----
  function openDownload() {
    downloadOpen.value = true;
  }
  function closeDownload() {
    downloadOpen.value = false;
  }

  // ---- 拖拽宽度 ----
  function setSplit(pct) {
    split.value = Math.min(50, Math.max(16, pct));
  }
  function persistSplit() {
    try {
      localStorage.setItem(LS_SPLIT, String(split.value));
    } catch {
      /* ignore */
    }
  }

  // 从 URL hash 读取一次性视图（set/active、sel、q、panel），便于分享链接与开发期验证。
  // 返回值：是否命中任意已知参数（供启动路由判断「深链优先」还是「按记忆路由」）。
  function loadFromHash() {
    if (typeof window === "undefined") return false;
    const p = new URLSearchParams(window.location.hash.slice(1));
    let applied = false;
    // set / active 同义
    const wantSet = p.has("set") ? p.get("set") : p.has("active") ? p.get("active") : null;
    if (wantSet !== null) {
      applied = true;
      if (wantSet === "all" || knowledgeSets.setById.get(wantSet)) {
        activeSetId.value = wantSet;
        search.clear();
        search.clearFilters();
        selectedUid.value = null;
      }
    }
    if (p.has("sel")) {
      const uid = p.get("sel");
      if (knowledgeSets.entryByUid.get(uid)) {
        selectedUid.value = uid;
        applied = true;
      }
    }
    if (p.has("q")) {
      search.q = p.get("q") ?? "";
      search.run();
      applied = true;
    }
    if (p.has("panel")) {
      const name = p.get("panel");
      if (name === "favorites") panel.value = "favorites";
      applied = true;
    }
    // 命中深链即表示用户意图进入工作区：无记忆时 view 初值可能停在 'home'，此处切到浏览态，
    // 否则深链只改了 activeSetId 却把用户留在首页。
    if (applied) view.value = "browse";
    return applied;
  }

  // ---- toast ----
  let toastTimer = null;
  function showToast(message, duration = 1800) {
    toast.value = { id: Date.now() + Math.random(), message };
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => {
      toast.value = null;
    }, duration);
  }

  return {
    view,
    activeSetId,
    selectedUid,
    split,
    themePref,
    systemTheme,
    resolvedTheme,
    panel,
    toast,
    setsManagerOpen,
    downloadOpen,
    activeSet,
    accentColor,
    selectedEntry,
    firstVisible,
    switchSet,
    gotoAll,
    gotoHome,
    gotoSettings,
    gotoBrowse,
    jumpToEntry,
    selectEntry,
    selectDefault,
    persistActiveSet,
    selectDiscipline,
    clearSelection,
    applyStartupRoute,
    toggleTheme,
    setThemePref,
    applyTheme,
    openPanel,
    closePanel,
    togglePanel,
    openSetsManager,
    closeSetsManager,
    openDownload,
    closeDownload,
    setSplit,
    persistSplit,
    loadFromHash,
    showToast,
  };
});
