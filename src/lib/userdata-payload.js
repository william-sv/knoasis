// Knoasis · userdata 迁移载荷纯函数（不依赖 window / IPC，可在沙箱 node 测试）
//
// 职责：把 localStorage 收藏/笔记快照转换为 user_migrate_legacy 载荷。
// 键名常量与 userdata.js 共用；userdata.js 只负责「何时迁」的编排。

export const LS_FAVORITES = "knoasis.favorites.v1";
export const LS_NOTES = "knoasis.notes.v1";
export const LS_MIGRATED = "knoasis.migrated.userdata.v1";

export function parseFavList(raw) {
  if (!raw) return [];
  try {
    const arr = JSON.parse(raw);
    return Array.isArray(arr) ? arr : [];
  } catch {
    return [];
  }
}

export function parseNotesRecord(raw) {
  if (!raw) return {};
  try {
    const obj = JSON.parse(raw);
    return obj && typeof obj === "object" ? obj : {};
  } catch {
    return {};
  }
}

/**
 * 纯函数：本地收藏/笔记快照 → user_migrate_legacy 载荷。
 * @param {Array} favs  localStorage favorites 数组（UI 快照）
 * @param {Object} notes localStorage notes 记录（Record<uid, {content, updatedAt}>）
 */
export function buildMigratePayload(favs, notes) {
  const favorites = (Array.isArray(favs) ? favs : []).map((f) => ({
    uid: String(f.uid ?? ""),
    name: String(f.name ?? ""),
    type: String(f.type ?? ""),
    discipline: String(f.discipline ?? ""),
    category: String(f.category ?? ""),
    level_code: String(f.levelCode ?? f.level_code ?? ""),
    summary: String(f.summary ?? ""),
  }));
  const noteObj =
    notes && typeof notes === "object" && !Array.isArray(notes) ? notes : {};
  const noteList = Object.entries(noteObj)
    .map(([uid, n]) => {
      const content = String(n?.content ?? "");
      const updatedAt = Number(n?.updatedAt ?? n?.updated_at ?? Date.now());
      return {
        uid: String(uid),
        content,
        updated_at: Number.isFinite(updatedAt) ? updatedAt : Date.now(),
      };
    })
    .filter((n) => n.uid);
  return { favorites, notes: noteList };
}
