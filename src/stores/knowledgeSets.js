// Pinia store：useKnowledgeSets
// 职责：知识集（学科）+ 条目数据的持有者与视图查询。
//
// 数据只来自 Rust IPC（学科包 = *.knowledgeset）：
//  - Tauri 内：knowledge_list_sets → registerSets(toSetShape(meta))；
//    再逐 set knowledge_list_entries({set_id}) → entries=map(EntryItem→UI)
//  - 非 Tauri（浏览器预览）或 IPC 失败：不加载任何假数据；置空数组 + source='empty'/'error'，
//    由界面呈现干净的空态/错误态（见 EntryList / StatusBar / TopToolbar）。
// store 对外 getter 全部不变（sets/entries/loaded/setById/entryByUid/totalCount/baseList/visibleEntries/visibleCount），
// 组件零改动（EntryList / FavoritesPanel / 搜索直接可用）。

import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { searchEntries } from "../lib/search.js";
import { isTauri, knowledge } from "../lib/ipc.js";
import { toEntryItem, toSetShape } from "../lib/grammar/adapter.js";
import { bootstrapUserData } from "../lib/userdata.js";
import * as registry from "../lib/data-registry.js";
import { useSearch } from "./search.js";
import { useUi } from "./ui.js";

export const useKnowledgeSets = defineStore("knowledgeSets", () => {
  // ---- state ----
  const sets = ref([]);
  const entries = ref([]);
  const loaded = ref(false);
  const loading = ref(false);
  const loadError = ref(""); // 非空 = 最近一次真实数据加载失败的提示文案
  const source = ref("empty"); // 'ipc'（真实数据已加载）| 'empty'（浏览器预览）| 'error'（IPC 加载失败）
  const dbInfo = ref(null); // knowledge_reload 返回 { sets, entries_total }

  // ---- init（幂等：只真正执行一次） ----
  let started = false;
  let promise = null;

  /** 清空注册表与 store（IPC 失败 / 浏览器预览共用），kind: 'empty' | 'error' */
  function clearData(kind, reason = "") {
    registry.registerSets([]);
    registry.registerEntries([]);
    sets.value = [];
    entries.value = [];
    loaded.value = true;
    loading.value = false;
    source.value = kind;
    loadError.value = reason;
  }

  function loadErrorText(e) {
    const code = e && e.code ? e.code : "IPC_ERROR";
    const msg = e && e.message ? e.message : "";
    if (code === "DB_UNAVAILABLE") {
      return msg
        ? `未找到学科包：${msg}。请在「管理学科包」中导入学科包`
        : "未找到学科包。请在「管理学科包」中导入学科包";
    }
    if (code === "SCHEMA_MISMATCH") {
      return `学科包 schema 与当前应用不兼容${msg ? `：${msg}` : ""}`;
    }
    return msg || "未知错误，请重试";
  }

  async function fetchRealData() {
    // 多学科：先 listSets，再逐 set 拉 listEntries({set_id})（Rust 按 set_id 路由包内 DB）
    const setMetas = await knowledge.listSets();
    const mappedSets = (setMetas || []).map(toSetShape);
    const mappedEntries = [];
    for (const s of mappedSets) {
      const listResult = await knowledge.listEntries({ set_id: s.id, limit: 5000 });
      for (const it of listResult?.items || []) {
        mappedEntries.push(toEntryItem(it, s.discipline));
      }
    }
    registry.registerSets(mappedSets);
    registry.registerEntries(mappedEntries);
    sets.value = mappedSets;
    entries.value = mappedEntries;
    loaded.value = true;
    source.value = "ipc";
    loadError.value = "";
  }

  async function loadReal() {
    loading.value = true;
    try {
      // 先取重载信息（含用户知识根路径），供设置页展示；失败不阻塞数据加载
      try {
        dbInfo.value = await knowledge.reload();
      } catch (e) {
        /* 信息缺失可忽略 */
      }
      await fetchRealData();
      loading.value = false;
      // 迁移链：知识集数据就绪后可判收藏悬空
      try {
        await bootstrapUserData();
      } catch (e) {
        // 收藏/笔记初始化失败不阻塞知识集展示（bootstrap 内部已 toast）
      }
      return { ok: true, info: dbInfo.value };
    } catch (e) {
      const reason = loadErrorText(e);
      clearData("error", reason);
      return { ok: false, error: reason, code: (e && e.code) || "IPC_ERROR" };
    }
  }

  function init() {
    if (started) return promise;
    started = true;
    if (!isTauri()) {
      // 浏览器预览：无假数据，干净空态（界面提示用桌面应用打开）
      clearData("empty", "");
      promise = Promise.resolve();
    } else {
      promise = loadReal();
    }
    return promise;
  }
  init();

  /**
   * 重新加载真实数据（供「管理学科包」操作后刷新与启动加载使用）。
   *  - source==='ipc'：先 knowledge.reload 让 Rust 重扫学科包，再重新拉取 sets+entries；
   *  - source==='error'：直接重试；重试再失败则保持 error 态并返回错误；
   *  - 非 Tauri（浏览器）：不支持，返回错误。
   * 重载失败不会清空当前已成功加载的数据（仅 toast 提示）。
   * @param {string|null} _path 兼容旧签名；knowledge 按根目录自动发现，忽略显式路径
   */
  async function reload(_path = null) {
    if (!isTauri()) {
      return { ok: false, error: "当前为浏览器预览，无法加载真实数据", code: "NOT_TAURI" };
    }
    try {
      if (source.value === "ipc") {
        const info = await knowledge.reload();
        dbInfo.value = info;
      }
      await fetchRealData();
      loading.value = false;
      return { ok: true, info: dbInfo.value };
    } catch (e) {
      const reason = loadErrorText(e);
      loading.value = false;
      if (source.value === "error") clearData("error", reason);
      return { ok: false, error: reason, code: (e && e.code) || "IPC_ERROR" };
    }
  }

  // ---- getters ----
  const setById = computed(() => {
    const m = new Map();
    sets.value.forEach((s) => m.set(s.id, s));
    return m;
  });

  const entryByUid = computed(() => {
    const m = new Map();
    entries.value.forEach((e) => m.set(e.uid, e));
    return m;
  });

  const totalCount = computed(() => entries.value.length);

  // 学科范围条目（不含搜索/筛选）：全部学科 or 当前学科
  const scopedEntries = computed(() => {
    const ui = useUi();
    if (ui.activeSetId === "all") return entries.value;
    return entries.value.filter((e) => e.discipline === ui.activeSetId);
  });

  // 搜索激活时 hits 优先；否则取当前学科 / 全部条目；再叠加属性筛选（AND）
  // 注：等级（level）概念已取消，不再作为筛选维度。
  const baseList = computed(() => {
    const search = useSearch();
    const f = search.filters;
    let list = search.active ? search.hits : scopedEntries.value;
    if (f.type != null) {
      list = list.filter((e) => e.type === f.type);
    }
    if (f.category != null) {
      list = list.filter((e) => e.category === f.category);
    }
    if (f.tag != null) {
      list = list.filter((e) => Array.isArray(e.tags) && e.tags.includes(f.tag));
    }
    return list;
  });

  // 可见列表（列表区数据源）：搜索 + 筛选（等级/类型/分类/标签）叠加后的结果
  const visibleEntries = computed(() => baseList.value);

  const visibleCount = computed(() => visibleEntries.value.length);

  return {
    sets,
    entries,
    loaded,
    loading,
    loadError,
    source,
    dbInfo,
    setById,
    entryByUid,
    totalCount,
    scopedEntries,
    baseList,
    visibleEntries,
    visibleCount,
    init,
    reload,
  };
});
