// Pinia store：useNotes
//
// 职责（对齐 docs/Knoasis-韩语语法接入设计.md §4.5.3）：条目笔记。
//  - Tauri：saveNote() 即时更新内存（getNote 仍同步），防抖 ~120ms 调 user_notes_save
//  - 纯浏览器 / 非 Tauri：沿用 localStorage
// store 对外接口不变：getNote / saveNote / count / map / notes

import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { isTauri, user } from "../lib/ipc.js";
import { useUi } from "./ui.js";

const LS_KEY = "knoasis.notes.v1";
const DEBOUNCE_MS = 120;

function loadLocal() {
  try {
    const raw = localStorage.getItem(LS_KEY);
    if (!raw) return {};
    const obj = JSON.parse(raw);
    return obj && typeof obj === "object" ? obj : {};
  } catch {
    return {};
  }
}

function toNoteRow(n) {
  return {
    uid: String(n.uid ?? ""),
    content: String(n.content ?? ""),
    updatedAt: Number(n.updated_at ?? n.updatedAt ?? 0) || Date.now(),
  };
}

export const useNotes = defineStore("notes", () => {
  const inTauri = isTauri();
  // Record<uid, { content, updatedAt }>
  const notes = ref(inTauri ? {} : loadLocal());

  let debounceTimer = null;
  let pendingSave = null; // 未落库的最新内容 { uid, content, updated_at }

  function persistLocal() {
    if (inTauri) return; // 迁移后不再双写 localStorage
    try {
      localStorage.setItem(LS_KEY, JSON.stringify(notes.value));
    } catch {
      /* 隐私模式忽略 */
    }
  }

  function flushNow() {
    if (debounceTimer) {
      clearTimeout(debounceTimer);
      debounceTimer = null;
    }
    if (!pendingSave) return;
    const payload = pendingSave;
    pendingSave = null;
    if (!inTauri) return;
    user.notesSave(payload).catch(() => {
      const ui = useUi();
      ui.showToast("笔记保存失败：本地用户数据不可用", 2400);
    });
  }

  /** 从 Rust 预填（迁移后 / 每次启动） */
  async function initFromRust() {
    if (!inTauri) {
      notes.value = loadLocal();
      return;
    }
    const rows = await user.notesList();
    const obj = {};
    for (const row of rows || []) {
      const n = toNoteRow(row);
      obj[n.uid] = { content: n.content, updatedAt: n.updatedAt };
    }
    notes.value = obj;
  }

  function getNote(uid) {
    const n = notes.value[uid];
    return n ? { uid, content: n.content, updatedAt: n.updatedAt } : null;
  }

  function saveNote(uid, content) {
    const text = String(content ?? "");
    const updatedAt = Date.now();
    notes.value[uid] = { content: text, updatedAt };
    persistLocal();

    if (!inTauri) return;
    // Tauri：防抖 ~120ms 落库；内容以最后一次输入为准
    pendingSave = { uid, content: text, updated_at: updatedAt };
    if (debounceTimer) clearTimeout(debounceTimer);
    debounceTimer = setTimeout(flushNow, DEBOUNCE_MS);
  }

  const count = computed(
    () => Object.keys(notes.value).filter((k) => (notes.value[k].content ?? "").trim()).length,
  );

  // 供导出使用：uid → { content, updatedAt }
  const map = computed(() => notes.value);

  return { notes, count, map, getNote, saveNote, initFromRust };
});
