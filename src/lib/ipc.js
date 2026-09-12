// Knoasis · Tauri IPC 封装
//
// 所有 Rust 命令的唯一调用入口（对齐 docs/Knoasis-学科数据组织与第三方接入方案.md §3.8）。
// 纯浏览器（无 window.__TAURI_INTERNALS__）下 isTauri() 返回 false；
// 本模块不做隐式降级，把“用哪条数据通路”的决策留给 store（浏览器为干净空态）。

import { invoke } from "@tauri-apps/api/core";

/** 是否运行在 Tauri webview 内（有 IPC 桥） */
export function isTauri() {
  if (typeof window === "undefined") return false;
  return Boolean(window.__TAURI_INTERNALS__);
}

/**
 * 归一化 Rust 命令错误 → { code, message }。
 * Rust ApiError 序列化为对象；panic/网络错误为字符串。
 */
export function normalizeError(e) {
  if (e && typeof e === "object" && typeof e.code === "string") {
    return { code: e.code, message: e.message || e.code };
  }
  if (e && typeof e === "object" && e.message) {
    return { code: "IPC_ERROR", message: String(e.message) };
  }
  const msg = typeof e === "string" ? e : String(e?.toString?.() || "未知错误");
  return { code: "IPC_ERROR", message: msg };
}

async function call(cmd, args) {
  try {
    return await invoke(cmd, args || {});
  } catch (e) {
    throw normalizeError(e);
  }
}

/**
 * knowledge 学科包命令（知识集入口，全部由学科包 meta 驱动）。
 *  - knowledge_list_sets()            → SubjectMeta[]（id/name/color/levels/types/template/counts/description）
 *  - knowledge_list_entries({...})    → { total, items }（EntryItem：uid/headword/category/level_code/level_label/tags/summary）
 *  - knowledge_get_entry({uid})       → EntryPayload（content 为 view-ready JSON 对象；images 已按 image_hidden 过滤）
 *  - knowledge_reload()               → { sets, entries_total }
 * 学科包管理（Dash 式 Docsets 管理）：
 *  - knowledge_manage_list()          → { user_root, sets: ManagedSet[] }（含已停用包）
 *  - knowledge_set_enabled({set_id, enabled}) → 同 manage_list（切换后回传最新列表）
 *  - knowledge_import_set({src_dir})  → ImportReport（校验 + 复制到用户知识根）
 *  - knowledge_remove_set({set_id})   → RemoveReport（用户包真删 / 内置包退化为停用）
 *  - knowledge_open_dir({dir})        → null（在系统文件管理器中打开用户知识根内的目录；越界报错）
 */
export const knowledge = {
  listSets: () => call("knowledge_list_sets"),
  listEntries: (payload = {}) => call("knowledge_list_entries", { args: payload }),
  getEntry: (uid) => call("knowledge_get_entry", { args: { uid } }),
  reload: () => call("knowledge_reload"),
  manageList: () => call("knowledge_manage_list"),
  setEnabled: (payload) => call("knowledge_set_enabled", { args: payload }),
  importSet: (payload) => call("knowledge_import_set", { args: payload }),
  removeSet: (payload) => call("knowledge_remove_set", { args: payload }),
  openDir: (dir) => call("knowledge_open_dir", { args: { dir } }),
};

/** userdata 命令 */
export const user = {
  favoritesList: () => call("user_favorites_list"),
  favoritesAdd: (input) => call("user_favorites_add", { input }),
  favoritesRemove: (uid) => call("user_favorites_remove", { args: { uid } }),
  notesList: () => call("user_notes_list"),
  notesSave: (input) => call("user_notes_save", { input }),
  migrateLegacy: (payload) => call("user_migrate_legacy", { args: payload }),
  // 图解隐藏（删除意愿落 userdata.image_hidden）；payload={entry_uid,img_path}
  imageHide: (payload) => call("user_image_hide", { args: payload }),
  // 恢复图解（预留）：payload={entry_uid,img_path}
  imageRestore: (payload) => call("user_image_restore", { args: payload }),
  // 复习（v3 userdata）：payload={uid, familiarity?}（familiarity 0-5，可选）
  reviewSet: (payload) => call("user_review_set", { args: payload }),
  reviewGet: (uid) => call("user_review_get", { args: { uid } }),
};

