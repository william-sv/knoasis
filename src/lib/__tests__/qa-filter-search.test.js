// QA 独立补充：筛选 × 搜索 组合逻辑（真实 Pinia store，非模拟）
// 运行：node --test src/lib/__tests__/qa-filter-search.test.js
//
// 覆盖实现者测试未涉及的点：
//  - baseList 中「搜索 ∩ 筛选」确为 AND
//  - setFilter 的 toggle 语义（同值第二次 = 取消）
//  - clearFilters 清空且不影响搜索词
//  - 「筛选 + 学科范围」组合
import { test, beforeEach } from "node:test";
import assert from "node:assert/strict";
import { createPinia, setActivePinia } from "pinia";
import { useKnowledgeSets } from "../../stores/knowledgeSets.js";
import { useSearch } from "../../stores/search.js";
import { useUi } from "../../stores/ui.js";

function entry(uid, name, { level = "", type = "grammar", category = "c", discipline = "set-x" } = {}) {
  return {
    uid,
    name,
    type,
    category,
    discipline,
    level: { code: level, rank: 0 },
    tags: [],
    summary: "",
    aliases: [],
    related: [],
    content: null,
  };
}

const FIXTURE = [
  entry("u1", "Alpha", { level: "L1", type: "grammar", category: "c1" }),
  entry("u2", "Alpha Beta", { level: "L2", type: "grammar", category: "c1" }),
  entry("u3", "Gamma", { level: "L1", type: "pos", category: "c2" }),
  entry("u4", "Delta", { level: "L1", type: "pos", category: "c2", discipline: "set-y" }),
];

function fresh() {
  setActivePinia(createPinia());
  const ks = useKnowledgeSets();
  const search = useSearch();
  const ui = useUi();
  ks.entries = FIXTURE.slice();
  ui.activeSetId = "all";
  return { ks, search, ui };
}

beforeEach(() => {
  // 每个用例独立 pinia
});

function uids(list) {
  return list.map((e) => e.uid).sort();
}

test("QA: 搜索 ∩ 筛选 为 AND（搜索命中 ∩ 等级筛选）", () => {
  const { ks, search } = fresh();
  search.q = "alpha";
  search.run();
  assert.deepEqual(uids(ks.visibleEntries), ["u1", "u2"], "仅搜索：两条 Alpha");

  search.setFilter("level", "L1");
  assert.deepEqual(uids(ks.visibleEntries), ["u1"], "搜索(alpha) ∩ 等级(L1) → 仅 u1");
});

test("QA: 仅筛选（无搜索）在学科范围内生效", () => {
  const { ks, search } = fresh();
  search.setFilter("type", "pos");
  assert.deepEqual(uids(ks.visibleEntries), ["u3", "u4"]);
  search.setFilter("category", "c2");
  assert.deepEqual(uids(ks.visibleEntries), ["u3", "u4"], "type=pos ∩ category=c2");
  search.setFilter("level", "L1");
  assert.deepEqual(uids(ks.visibleEntries), ["u3", "u4"]);
});

test("QA: setFilter 同值第二次点击 = 取消该维度（toggle）", () => {
  const { ks, search } = fresh();
  search.setFilter("level", "L1");
  assert.equal(search.filters.level, "L1");
  assert.deepEqual(uids(ks.visibleEntries), ["u1", "u3", "u4"]);

  search.setFilter("level", "L1"); // 再次点击同值
  assert.equal(search.filters.level, null);
  assert.deepEqual(uids(ks.visibleEntries), ["u1", "u2", "u3", "u4"]);
});

test("QA: 切换到另一维度的值会替换而非并存", () => {
  const { search } = fresh();
  search.setFilter("level", "L1");
  search.setFilter("level", "L2");
  assert.equal(search.filters.level, "L2");
  assert.equal(search.filterActive, true);
});

test("QA: clearFilters 清空全部维度但不影响搜索词", () => {
  const { search } = fresh();
  search.q = "alpha";
  search.run();
  search.setFilter("level", "L2");
  search.setFilter("type", "grammar");
  assert.equal(search.filterActive, true);

  search.clearFilters();
  assert.deepEqual(search.filters, { level: null, type: null, category: null });
  assert.equal(search.filterActive, false);
  assert.equal(search.q, "alpha", "搜索词不应被清除");
  assert.equal(search.active, true);
});

test("QA: 搜索 + 筛选 + 学科范围 三者叠加", () => {
  const { ks, search, ui } = fresh();
  ui.activeSetId = "set-x"; // 排除 u4（set-y）
  search.q = "a"; // 命中 Alpha/Alpha Beta/Gamma/Delta
  search.run();
  search.setFilter("type", "pos");
  // set-x 下 type=pos 且含 'a' → u3（Gamma）；u4 属 set-y 被排除
  assert.deepEqual(uids(ks.visibleEntries), ["u3"]);
});

test("QA: 未知筛选键被忽略，不产生幽灵筛选", () => {
  const { search } = fresh();
  search.setFilter("bogus", "x");
  assert.deepEqual(search.filters, { level: null, type: null, category: null });
  assert.equal(search.filterActive, false);
});

test("QA: 空搜索词时不进入搜索态，visible 由筛选决定", () => {
  const { ks, search } = fresh();
  search.q = "   ";
  search.run();
  assert.equal(search.active, false);
  assert.deepEqual(search.hits, []);
  search.setFilter("category", "c1");
  assert.deepEqual(uids(ks.visibleEntries), ["u1", "u2"]);
});
