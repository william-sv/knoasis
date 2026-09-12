// Pinia store：useFavorites
//
// 职责（对齐 docs/Knoasis-韩语语法接入设计.md §4.5.3）：收藏状态。
//  - Tauri：内存为实时源 + 异步 Rust userdata.db 后端；toggle 乐观更新，失败回滚 + toast
//  - 纯浏览器 / 非 Tauri：沿用 localStorage 持久化
// store 对外接口不变：isFavorite / toggle / list / count / remove / items
// （list 项追加 set/missing 展示元信息，组件消费向后兼容）。

import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { isTauri, user } from "../lib/ipc.js";
import { findSetById } from "../lib/data-registry.js";
import { useKnowledgeSets } from "./knowledgeSets.js";
import { useUi } from "./ui.js";

const LS_KEY = "knoasis.favorites.v1";

function loadLocal() {
  try {
    const raw = localStorage.getItem(LS_KEY);
    if (!raw) return [];
    const arr = JSON.parse(raw);
    return Array.isArray(arr) ? arr : [];
  } catch {
    return [];
  }
}

function toUiRow(row) {
  return {
    uid: String(row.uid ?? ""),
    name: String(row.name ?? ""),
    type: String(row.type ?? ""),
    discipline: String(row.discipline ?? ""),
    category: String(row.category ?? ""),
    levelCode: String(row.level_code ?? ""),
    summary: String(row.summary ?? ""),
    favoritedAt: Number(row.created_at ?? 0) || Date.now(),
  };
}

/** UI 快照 → Rust FavoriteInput（snake_case） */
function toFavoriteInput(s) {
  return {
    uid: String(s.uid ?? ""),
    name: String(s.name ?? ""),
    type: String(s.type ?? ""),
    discipline: String(s.discipline ?? ""),
    category: String(s.category ?? ""),
    level_code: String(s.levelCode ?? ""),
    summary: String(s.summary ?? ""),
  };
}

export const useFavorites = defineStore("favorites", () => {
  const inTauri = isTauri();
  // [{ uid, name, type, discipline, category, levelCode, summary, favoritedAt }]
  const items = ref(inTauri ? [] : loadLocal());

  function persist() {
    if (inTauri) return; // 迁移后不再双写 localStorage
    try {
      localStorage.setItem(LS_KEY, JSON.stringify(items.value));
    } catch {
      /* 隐私模式忽略 */
    }
  }

  const uidSet = computed(() => new Set(items.value.map((i) => i.uid)));

  function isFavorite(uid) {
    return uidSet.value.has(uid);
  }

  /**
   * 从 Rust 预填收藏（迁移后 / 每次启动）。
   * 浏览器路径：从 localStorage 重新读取（幂等）。
   */
  async function initFromRust() {
    if (!inTauri) {
      items.value = loadLocal();
      return;
    }
    const rows = await user.favoritesList();
    items.value = (rows || []).map(toUiRow);
  }

  // 收藏/取消收藏；entry 为完整条目对象，用于写快照。返回是否处于已收藏态
  function toggle(entry) {
    if (!entry) return false;
    const idx = items.value.findIndex((i) => i.uid === entry.uid);
    if (idx >= 0) {
      const [prev] = items.value.splice(idx, 1);
      persist();
      if (inTauri) {
        user.favoritesRemove(entry.uid).catch(() => {
          items.value.splice(idx, 0, prev);
          persist();
          const ui = useUi();
          ui.showToast("取消收藏失败：本地用户数据不可用", 2400);
        });
      }
      return false;
    }
    const snapshot = {
      uid: entry.uid,
      name: entry.name,
      type: entry.type,
      discipline: entry.discipline,
      category: entry.category ?? "",
      levelCode: entry.level ? entry.level.code : "",
      summary: entry.summary,
      favoritedAt: Date.now(),
    };
    items.value.push(snapshot);
    persist();
    if (inTauri) {
      user.favoritesAdd(toFavoriteInput(snapshot)).catch(() => {
        const i = items.value.findIndex((x) => x.uid === snapshot.uid);
        if (i >= 0) items.value.splice(i, 1);
        persist();
        const ui = useUi();
        ui.showToast("收藏失败：本地用户数据不可用", 2400);
      });
    }
    return true;
  }

  function remove(uid) {
    const idx = items.value.findIndex((i) => i.uid === uid);
    if (idx < 0) return;
    const [prev] = items.value.splice(idx, 1);
    persist();
    if (inTauri) {
      user.favoritesRemove(uid).catch(() => {
        items.value.splice(idx, 0, prev);
        persist();
        const ui = useUi();
        ui.showToast("取消收藏失败：本地用户数据不可用", 2400);
      });
    }
  }

  // 带展示元信息的列表（按收藏时间倒序）；missing = 当前条目集已不含该 uid（悬空收藏）
  const list = computed(() => {
    const ks = useKnowledgeSets();
    return items.value
      .map((item) => ({
        ...item,
        set: findSetById(item.discipline),
        missing: !ks.entryByUid.get(item.uid),
      }))
      .sort((a, b) => b.favoritedAt - a.favoritedAt);
  });

  const count = computed(() => items.value.length);

  return { items, list, count, isFavorite, toggle, remove, initFromRust };
});
