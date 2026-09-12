// QA 独立边界补充（不改动实现者测试）
// 运行：node --test src/lib/__tests__/qa-startup-boundary.test.js
//
// 目的：用挑剔视角验证「启动记忆」在异常输入下不会崩、行为可预期：
//  - 记忆值被手工篡改为不存在的 id
//  - 记忆值是「损坏 JSON / 任意脏字符串」
//  - 存储返回非字符串（模拟异常实现）
//  - 存储抛错 / 存储缺失
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  LS_ACTIVE_SET,
  readActiveSetPref,
  writeActiveSetPref,
  clearActiveSetPref,
  resolveStartupView,
} from "../startup.js";

function makeStorage(initial = {}) {
  const map = new Map(Object.entries(initial));
  return {
    getItem: (k) => (map.has(k) ? map.get(k) : null),
    setItem: (k, v) => void map.set(k, String(v)),
    removeItem: (k) => void map.delete(k),
    _dump: () => Object.fromEntries(map),
  };
}

const SETS = [{ id: "ko-grammar" }, { id: "en-grammar" }];

test("QA: 记忆被篡改为不存在的 id → 回落首页且判定需清键", () => {
  const r = resolveStartupView("does-not-exist", SETS);
  assert.deepEqual(r, { view: "home", activeSetId: "all", clearMemory: true });
});

test("QA: 记忆是损坏 JSON 字符串 → 不崩，按未知 id 回落首页并清键", () => {
  const storage = makeStorage({ [LS_ACTIVE_SET]: '{"id":' }); // 半截 JSON
  const remembered = readActiveSetPref(storage); // 故意当普通字符串处理
  assert.equal(remembered, '{"id":');
  const r = resolveStartupView(remembered, SETS);
  assert.equal(r.view, "home");
  assert.equal(r.clearMemory, true);
  // 不抛异常
});

test("QA: 记忆是任意脏字符串（含空格/emoji/超长）→ 安全的 home 回落", () => {
  for (const dirty of ["   ", "null", "undefined", "🚀", "a".repeat(4096), "\u0000"]) {
    const r = resolveStartupView(dirty, SETS);
    assert.equal(r.view, "home", `dirty=${JSON.stringify(dirty.slice(0, 12))}`);
    assert.equal(r.clearMemory, true);
  }
  // 空串视为「无记忆」（不触发清键）
  assert.deepEqual(resolveStartupView("", SETS), {
    view: "home",
    activeSetId: "all",
    clearMemory: false,
  });
});

test("QA: 存储返回非字符串（坏实现）→ readActiveSetPref 归零为 null", () => {
  const weird = {
    getItem: () => ({ not: "a string" }),
    setItem: () => {},
    removeItem: () => {},
  };
  assert.equal(readActiveSetPref(weird), null);
  assert.equal(readActiveSetPref({ getItem: () => 0, setItem() {}, removeItem() {} }), null);
  assert.equal(readActiveSetPref({ getItem: () => "", setItem() {}, removeItem() {} }), null);
});

test("QA: writeActiveSetPref 的返回值与落盘一致性（含 'all'）", () => {
  const storage = makeStorage();
  assert.equal(writeActiveSetPref("all", storage), true);
  assert.equal(storage._dump()[LS_ACTIVE_SET], "all");
  // 非法 id（空/假值）不写入，且不影响已有值
  assert.equal(writeActiveSetPref("", storage), false);
  assert.equal(writeActiveSetPref(null, storage), false);
  assert.equal(storage._dump()[LS_ACTIVE_SET], "all");
});

test("QA: 无存储（未注入且无 window.localStorage）时读写均安全返回", () => {
  // node 环境没有 window → getDefaultStorage 返回 null
  assert.equal(readActiveSetPref(), null);
  assert.equal(writeActiveSetPref("all"), false);
  assert.equal(clearActiveSetPref(), false);
});

test("QA: 篡改后经真实「读→判定→清」链路，键最终被移除", () => {
  const storage = makeStorage({ [LS_ACTIVE_SET]: "ghost-set" });
  const remembered = readActiveSetPref(storage);
  const r = resolveStartupView(remembered, SETS);
  assert.equal(r.clearMemory, true);
  if (r.clearMemory) clearActiveSetPref(storage);
  assert.equal(storage.getItem(LS_ACTIVE_SET), null, "失效记忆应被清除");
});

test("QA: sets 里含 null/畸形元素时不崩（防御性）", () => {
  const messy = [null, undefined, { id: "en-grammar" }, {}];
  assert.deepEqual(resolveStartupView("en-grammar", messy), {
    view: "browse",
    activeSetId: "en-grammar",
    clearMemory: false,
  });
});
