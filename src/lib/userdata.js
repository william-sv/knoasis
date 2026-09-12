// Knoasis · localStorage → Rust userdata.db 单向迁移
//
// 流程（对齐 docs/Knoasis-韩语语法接入设计.md §4.5.3）：
//  1. 仅 Tauri 内执行（浏览器继续 localStorage 兜底，不迁）
//  2. 读 localStorage knoasis.favorites.v1 / knoasis.notes.v1
//  3. 尚无 knoasis.migrated.userdata.v1 标记且任一非空 → user_migrate_legacy
//  4. 成功 → 写标记 + 清空两条 localStorage；服务端 ON CONFLICT 保证重跑不重复
//  5. 迁移后不再双写 localStorage（Rust userdata.db 是 Tauri 内唯一持久源）
//
// 载荷纯函数见 userdata-payload.js（便于沙箱 node 测试，本文件不直接 import @tauri-apps/api 以外的副作用）。

import { isTauri, user } from "./ipc.js";
import { useFavorites } from "../stores/favorites.js";
import { useNotes } from "../stores/notes.js";
import { useUi } from "../stores/ui.js";
import {
  LS_FAVORITES,
  LS_NOTES,
  LS_MIGRATED,
  parseFavList,
  parseNotesRecord,
  buildMigratePayload,
} from "./userdata-payload.js";

export {
  LS_FAVORITES,
  LS_NOTES,
  LS_MIGRATED,
  buildMigratePayload,
};

function safeGet(key) {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}
function safeSet(key, val) {
  try {
    localStorage.setItem(key, val);
  } catch {
    /* 隐私模式忽略 */
  }
}
function safeRemove(key) {
  try {
    localStorage.removeItem(key);
  } catch {
    /* ignore */
  }
}

/**
 * 幂等单向迁移 + 初始化收藏/笔记 store（从 Rust 预填）。
 * 返回 true 表示完成（含“无需迁移”）；false 表示 userdata 不可用。
 * 由 knowledgeSets.init 末尾链式调用，保证 grammar 数据先就绪可判 missing。
 */
export async function bootstrapUserData() {
  if (!isTauri()) return false;
  const favorites = useFavorites();
  const notes = useNotes();

  const legacyFav = parseFavList(safeGet(LS_FAVORITES));
  const legacyNotes = parseNotesRecord(safeGet(LS_NOTES));
  const already = safeGet(LS_MIGRATED) === "1";
  const hasLegacy = legacyFav.length > 0 || Object.keys(legacyNotes).length > 0;

  if (!already) {
    if (hasLegacy) {
      try {
        await user.migrateLegacy(buildMigratePayload(legacyFav, legacyNotes));
        safeSet(LS_MIGRATED, "1");
        safeRemove(LS_FAVORITES);
        safeRemove(LS_NOTES);
      } catch (e) {
        const ui = useUi();
        const msg =
          e && e.code === "USER_DB_UNAVAILABLE"
            ? "本地用户数据不可用，请检查磁盘权限"
            : "收藏/笔记迁移失败，本次暂用内存态";
        ui.showToast(msg, 2600);
        return false;
      }
    } else {
      // 无旧数据也写标记，避免每次启动都尝试读取
      safeSet(LS_MIGRATED, "1");
    }
  }

  try {
    await Promise.all([favorites.initFromRust(), notes.initFromRust()]);
  } catch (e) {
    const ui = useUi();
    ui.showToast("本地用户数据读取失败，收藏与笔记暂为空", 2600);
    return false;
  }
  return true;
}
