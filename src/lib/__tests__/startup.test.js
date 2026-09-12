// 沙箱验证：启动路由（记忆学科包）
// 运行：node --test src/lib/__tests__/startup.test.js
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  LS_ACTIVE_SET,
  readActiveSetPref,
  writeActiveSetPref,
  clearActiveSetPref,
  resolveStartupView,
} from "../startup.js";

/** 内存版 Storage 桩 */
function makeStorage(initial = {}) {
  const map = new Map(Object.entries(initial));
  return {
    getItem: (k) => (map.has(k) ? map.get(k) : null),
    setItem: (k, v) => {
      map.set(k, String(v));
    },
    removeItem: (k) => {
      map.delete(k);
    },
  };
}

const SETS = [{ id: "kr-grammar" }, { id: "en-grammar" }];

// ---- resolveStartupView：核心三态 + 边界 ----

test("无记忆 → 首页（首次启动）", () => {
  const r = resolveStartupView(null, SETS);
  assert.equal(r.view, "home");
  assert.equal(r.activeSetId, "all");
  assert.equal(r.clearMemory, false);
});

test("记忆学科仍存在 → 直达该学科浏览页", () => {
  const r = resolveStartupView("en-grammar", SETS);
  assert.equal(r.view, "browse");
  assert.equal(r.activeSetId, "en-grammar");
  assert.equal(r.clearMemory, false);
});

test("记忆学科已被删除/停用 → 回落首页并清记忆", () => {
  const r = resolveStartupView("ja-grammar", SETS);
  assert.equal(r.view, "home");
  assert.equal(r.activeSetId, "all");
  assert.equal(r.clearMemory, true);
});

test("记忆「全部」→ 浏览全部（视为有效选择）", () => {
  const r = resolveStartupView("all", SETS);
  assert.equal(r.view, "browse");
  assert.equal(r.activeSetId, "all");
  assert.equal(r.clearMemory, false);
});

test("无记忆且无任何学科包 → 首页（导入引导）", () => {
  const r = resolveStartupView(null, []);
  assert.equal(r.view, "home");
  assert.equal(r.clearMemory, false);
});

test("记忆学科但已无任何学科包 → 首页并清失效记忆", () => {
  const r = resolveStartupView("kr-grammar", []);
  assert.equal(r.view, "home");
  assert.equal(r.activeSetId, "all");
  assert.equal(r.clearMemory, true);
});

test("记忆「全部」但已无学科包 → 首页（保留记忆）", () => {
  const r = resolveStartupView("all", []);
  assert.equal(r.view, "home");
  assert.equal(r.clearMemory, false);
});

test("sets 非数组时按空处理，不抛错", () => {
  const r = resolveStartupView("kr-grammar", undefined);
  assert.equal(r.view, "home");
  assert.equal(r.clearMemory, true);
});

// ---- 记忆读写：localStorage 键 knoasis.activeSet.v1 ----

test("写入 / 读取 / 清除记忆（含「全部」）", () => {
  const storage = makeStorage();
  assert.equal(readActiveSetPref(storage), null);

  assert.equal(writeActiveSetPref("en-grammar", storage), true);
  assert.equal(readActiveSetPref(storage), "en-grammar");
  assert.equal(storage.getItem(LS_ACTIVE_SET), "en-grammar");

  // 「全部」也应被记住
  assert.equal(writeActiveSetPref("all", storage), true);
  assert.equal(readActiveSetPref(storage), "all");

  assert.equal(clearActiveSetPref(storage), true);
  assert.equal(readActiveSetPref(storage), null);
});

test("存储不可用（存储抛错）时读写安全降级", () => {
  const broken = {
    getItem: () => {
      throw new Error("blocked");
    },
    setItem: () => {
      throw new Error("blocked");
    },
    removeItem: () => {
      throw new Error("blocked");
    },
  };
  assert.equal(readActiveSetPref(broken), null);
  assert.equal(writeActiveSetPref("all", broken), false);
  assert.equal(clearActiveSetPref(broken), false);
  // 空 id 不写入
  assert.equal(writeActiveSetPref("", makeStorage()), false);
});
