// QA 收尾轮补充：证明 P1「记忆误清」是真修好（而非仅让测试过），并核查深链副作用。
// 运行：node --test src/lib/__tests__/qa-fix-verify.test.js
//
// 说明：刻意新建文件，不改动已入库的 3 个 QA 文件（便于比对工程师是否偷改断言）。
import { test, afterEach } from "node:test";
import assert from "node:assert/strict";
import { createPinia, setActivePinia } from "pinia";
import { useUi } from "../../stores/ui.js";
import { useKnowledgeSets } from "../../stores/knowledgeSets.js";
import { LS_ACTIVE_SET } from "../startup.js";

function makeStorage(initial = {}) {
  const map = new Map(Object.entries(initial));
  return {
    getItem: (k) => (map.has(k) ? map.get(k) : null),
    setItem: (k, v) => void map.set(k, String(v)),
    removeItem: (k) => void map.delete(k),
    has: (k) => map.has(k),
  };
}

function boot(memoryValue, { source, sets, hash } = {}) {
  const storage = makeStorage(memoryValue == null ? {} : { [LS_ACTIVE_SET]: memoryValue });
  global.window = { localStorage: storage, matchMedia: undefined, location: { hash: hash ?? "" } };
  setActivePinia(createPinia());
  const ks = useKnowledgeSets();
  const ui = useUi();
  ks.entries = [];
  ks.sets = sets ?? [];
  ks.source = source ?? "ipc";
  ks.loaded = true;
  return { ui, ks, storage };
}

afterEach(() => delete global.window);

// ---------------- P1：区分「该保留」与「该清」 ----------------

test("修后①：加载失败 → 保留记忆 + 回落首页", () => {
  const { ui, storage } = boot("en-grammar", { source: "error", sets: [] });
  ui.applyStartupRoute();
  assert.equal(ui.view, "home");
  assert.equal(storage.getItem(LS_ACTIVE_SET), "en-grammar", "error 态必须保留记忆");
});

test("修后②：浏览器空态(empty) → 同样保留记忆（不误清）", () => {
  const { ui, storage } = boot("en-grammar", { source: "empty", sets: [] });
  ui.applyStartupRoute();
  assert.equal(ui.view, "home");
  assert.equal(storage.getItem(LS_ACTIVE_SET), "en-grammar", "empty 态必须保留记忆");
});

test("修后③：真实加载成功(ipc) 且包里确实没该 id → 仍清键（真失效不能漏清）", () => {
  const { ui, storage } = boot("ja-grammar", { source: "ipc", sets: [{ id: "en-grammar" }] });
  ui.applyStartupRoute();
  assert.equal(ui.view, "home");
  assert.equal(storage.getItem(LS_ACTIVE_SET), null, "ipc 下确无该包 → 应清失效记忆");
});

test("修后④：error 且从未有过记忆 → 不写入任何脏值", () => {
  const { ui, storage } = boot(null, { source: "error", sets: [] });
  ui.applyStartupRoute();
  assert.equal(ui.view, "home");
  assert.equal(storage.has(LS_ACTIVE_SET), false, "没有记忆就不应有键被创建");
});

test("修后⑤：失败后 reload 成功 → 再次路由，记忆恢复生效进入该学科", () => {
  const { ui, ks, storage } = boot("en-grammar", { source: "error", sets: [] });
  ui.applyStartupRoute();
  assert.equal(ui.view, "home");
  assert.equal(storage.getItem(LS_ACTIVE_SET), "en-grammar");

  // 模拟 reload 成功
  ks.sets = [{ id: "en-grammar" }, { id: "ko-grammar" }];
  ks.source = "ipc";
  ui.applyStartupRoute();
  assert.equal(ui.view, "browse", "成功加载后应恢复进入该学科");
  assert.equal(ui.activeSetId, "en-grammar");
  assert.equal(storage.getItem(LS_ACTIVE_SET), "en-grammar");
});

test("修后⑥：序列 失败→成功→包被真删，清键只在最后一步发生", () => {
  const { ui, ks, storage } = boot("en-grammar", { source: "error", sets: [] });
  ui.applyStartupRoute();
  assert.equal(storage.getItem(LS_ACTIVE_SET), "en-grammar"); // 失败：保留
  ks.source = "ipc";
  ks.sets = [{ id: "en-grammar" }];
  ui.applyStartupRoute();
  assert.equal(storage.getItem(LS_ACTIVE_SET), "en-grammar"); // 存在：保留
  ks.sets = []; // 包被删除
  ui.applyStartupRoute();
  assert.equal(storage.getItem(LS_ACTIVE_SET), null); // 真失效：清
  assert.equal(ui.view, "home");
});

// ---------------- P2-2：深链副作用核查 ----------------

test("深链①：无关 hash 参数不应清掉/覆盖有效记忆键，但 deep-link 会抑制记忆落地（现存行为）", () => {
  const { ui, ks, storage } = boot("en-grammar", {
    source: "ipc",
    sets: [{ id: "en-grammar" }],
    hash: "#panel=favorites",
  });
  // 复刻 App.vue：命中深链则不调用 applyStartupRoute
  const deepLinked = ui.loadFromHash();
  if (!deepLinked) ui.applyStartupRoute();
  assert.equal(deepLinked, true);
  assert.equal(ui.view, "browse", "有参数 → 进入浏览态（本次修复点）");
  assert.equal(storage.getItem(LS_ACTIVE_SET), "en-grammar", "深链不应清掉记忆键");
  // 观察：因 deepLinked 抑制了 applyStartupRoute，activeSetId 未按记忆落到 en-grammar
  assert.equal(ui.activeSetId, "all", "（观察）无关 hash 会吞掉记忆学科的落地，落到 全部");
});

test("深链②：无 hash 参数时不干扰记忆落地（deepLinked=false）", () => {
  const { ui, storage } = boot("en-grammar", {
    source: "ipc",
    sets: [{ id: "en-grammar" }],
    hash: "",
  });
  const deepLinked = ui.loadFromHash();
  if (!deepLinked) ui.applyStartupRoute();
  assert.equal(deepLinked, false);
  assert.equal(ui.view, "browse");
  assert.equal(ui.activeSetId, "en-grammar", "无深链时应按记忆落学科");
  assert.equal(storage.getItem(LS_ACTIVE_SET), "en-grammar");
});

test("深链③：loadFromHash 无任何已知参数时 applied=false，不改动 view", () => {
  const { ui } = boot("en-grammar", { source: "ipc", sets: [{ id: "en-grammar" }], hash: "#foo=bar" });
  ui.view = "settings";
  const applied = ui.loadFromHash();
  assert.equal(applied, false, "未知参数不算深链");
  assert.equal(ui.view, "settings", "无参数不应强切浏览态");
});

test("深链④：命中合法 set 时切到浏览态且 activeSetId 正确", () => {
  const { ui, ks } = boot(null, {
    source: "ipc",
    sets: [{ id: "en-grammar" }],
    hash: "#set=en-grammar",
  });
  const applied = ui.loadFromHash();
  assert.equal(applied, true);
  assert.equal(ui.view, "browse");
  assert.equal(ui.activeSetId, "en-grammar");
});
