// Knoasis · 运行时学科注册表
//
// 目的（对齐 docs/Knoasis-学科数据组织与第三方接入方案.md §3.1）：
//  App 数据只来自 Rust IPC（学科包 knowledge_list_sets / knowledge_list_entries）。
//  knowledgeSets.js init 成功接 IPC 后，
//  调用 registerSets/registerEntries 注册「当前生效」的 sets/entries；
//  findSetById/levelMetaForEntry 等查询函数统一读本模块，避免组件直接接触 IPC 形状。
//
// 生命周期：
//  - knowledgeSets.js init（Tauri）成功 → registerSets/registerEntries（真实数据）
//  - IPC 失败 / 非 Tauri 浏览器：store 置空注册（registerSets([])），界面走空态/错误态。

const state = {
  sets: [],
  setsById: new Map(),
  entries: [],
  entriesByUid: new Map(),
};

function rebuildSetsIndex() {
  const m = new Map();
  state.sets.forEach((s) => m.set(s.id, s));
  state.setsById = m;
}

function rebuildEntriesIndex() {
  const m = new Map();
  state.entries.forEach((e) => m.set(e.uid, e));
  state.entriesByUid = m;
}

/** 替换当前生效的学科集合（真实数据 / 清空均走这里） */
export function registerSets(arr) {
  state.sets = Array.isArray(arr) ? arr : [];
  rebuildSetsIndex();
}

/** 替换当前生效的条目集合 */
export function registerEntries(arr) {
  state.entries = Array.isArray(arr) ? arr : [];
  rebuildEntriesIndex();
}

export function getSets() {
  return state.sets;
}

export function getEntries() {
  return state.entries;
}

/** 查学科；未注册（含空注册）返回 undefined */
export function findSetById(setId) {
  return state.setsById.get(setId);
}

/** 查条目；未注册（含空注册）返回 undefined */
export function findEntryByUid(uid) {
  return state.entriesByUid.get(uid);
}

/**
 * 取条目所属学科的 level 元信息（展示 label）。
 * 未分级（code '' / NULL）不在 set.levels 中，这里做兜底返回「未分级」。
 * 学科未注册时不抛错：分级 code 回落为 code 本身，空 code 回落为「未分级」。
 */
export function levelMetaForEntry(entry) {
  if (!entry) return { code: "", label: "", rank: 0 };
  const set = findSetById(entry.discipline);
  const code = entry.level && entry.level.code != null ? entry.level.code : "";
  if (set) {
    const lv = set.levels.find((l) => l.code === code);
    if (lv) return lv;
  }
  if (code === "" || code == null) {
    return { code: "", label: "未分级", rank: 0 };
  }
  return { code, label: code, rank: (entry.level && entry.level.rank) || 0 };
}
