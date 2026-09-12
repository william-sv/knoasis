// QA 独立复现：启动路由与「学科包加载失败」的竞态（真实 ui store）
// 运行：node --test src/lib/__tests__/qa-startup-route-error.test.js
//
// 背景：applyStartupRoute 在 ks.loaded 变 true 后执行。
//   - 列表「加载成功但零包」（source='ipc'/'empty'，sets=[]）→ 清失效记忆：符合预期
//   - 列表「加载失败」（source='error'，sets=[]）→ 也走到同一分支！
// 期望：加载失败不应把「本来有效的记忆」当失效清掉（否则用户下次启动会莫名回到首页）。
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
  };
}

function setup(memoryValue, { source, sets }) {
  const storage = makeStorage(memoryValue == null ? {} : { [LS_ACTIVE_SET]: memoryValue });
  global.window = { localStorage: storage, matchMedia: undefined };
  setActivePinia(createPinia());
  const ks = useKnowledgeSets(); // 非 Tauri：init → clearData('empty')
  const ui = useUi();
  ks.entries = [];
  ks.sets = sets;
  ks.source = source;
  ks.loaded = true;
  ui.applyStartupRoute();
  return { ui, ks, storage };
}

afterEach(() => {
  delete global.window;
});

test("QA 对照：加载成功、记忆学科存在 → 进入该学科浏览页并保留记忆", () => {
  const { ui, storage } = setup("en-grammar", {
    source: "ipc",
    sets: [{ id: "en-grammar" }, { id: "ko-grammar" }],
  });
  assert.equal(ui.view, "browse");
  assert.equal(ui.activeSetId, "en-grammar");
  assert.equal(storage.getItem(LS_ACTIVE_SET), "en-grammar", "有效记忆应保留");
});

test("QA 对照：加载成功但零包（包被删）→ 回落首页并清记忆（符合预期）", () => {
  const { ui, storage } = setup("en-grammar", { source: "ipc", sets: [] });
  assert.equal(ui.view, "home");
  assert.equal(storage.getItem(LS_ACTIVE_SET), null, "包已删除 → 清失效记忆");
});

// ⚠ 期望失败：证明「加载失败会误清有效记忆」这一缺陷
test("QA-BUG：加载失败(source=error) 不应清掉有效记忆", () => {
  const { ui, storage } = setup("en-grammar", { source: "error", sets: [] });
  assert.equal(ui.view, "home", "失败时停留首页是合理的");
  assert.equal(
    storage.getItem(LS_ACTIVE_SET),
    "en-grammar",
    "[缺陷] 加载失败把有效记忆误判为失效并清除；下次成功启动会莫名回到首页",
  );
});

// 深链优先：无记忆 + hash 指定学科，App.vue 走的是 loadFromHash 分支（view 保持初始值）
test("QA：无记忆时 hash 深链应把用户带到浏览页（当前未切换）", () => {
  const storage = makeStorage();
  global.window = {
    localStorage: storage,
    matchMedia: undefined,
    location: { hash: "#set=en-grammar" },
  };
  setActivePinia(createPinia());
  const ks = useKnowledgeSets();
  const ui = useUi(); // 无记忆 → view 初值 'home'
  ks.sets = [{ id: "en-grammar" }];
  ks.source = "ipc";

  const deep = ui.loadFromHash(); // 模拟 App.vue：命中断链则不调用 applyStartupRoute
  assert.equal(deep, true);
  assert.equal(ui.activeSetId, "en-grammar", "深链已解析出 activeSetId");
  assert.equal(ui.view, "browse", "[缺陷?] 无记忆时深链未切换到浏览页，用户停在首页");
});
