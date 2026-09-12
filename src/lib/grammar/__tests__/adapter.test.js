// 沙箱验证：adapter 映射 / 模板渲染键对齐 / 导出 Markdown（对齐方案 §7.1）
// 运行：node --test src/lib/grammar/__tests__/adapter.test.js
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

import {
  toEntryItem,
  toSetShape,
  toDetailView,
  rankOf,
  levelLabelOf,
} from "../adapter.js";
import { exportGrammarMarkdown } from "../export.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "../../../..");

function loadFixture(name) {
  return JSON.parse(
    readFileSync(path.join(__dirname, "../fixtures", name), "utf8"),
  );
}

function loadRealTemplate() {
  const raw = readFileSync(
    path.join(repoRoot, "src-tauri/resources/grammar/template_registry.json"),
    "utf8",
  );
  return JSON.parse(raw).kr_grammar;
}

test("rankOf / levelLabelOf 映射 I/II/''", () => {
  assert.equal(rankOf("I"), 1);
  assert.equal(rankOf("II"), 2);
  assert.equal(rankOf(""), 0);
  assert.equal(rankOf(null), 0);
  assert.equal(levelLabelOf("I"), "TOPIK I");
  assert.equal(levelLabelOf("II"), "TOPIK II");
  assert.equal(levelLabelOf(""), "未分级");
});

test("toEntryItem 映射真实 N마저（未分级：level '' rank 0、category 助词）", () => {
  const { items } = loadFixture("entry-list.sample.json");
  const ui = toEntryItem(items[0]);
  assert.equal(ui.uid, items[0].uid);
  assert.equal(ui.name, "N마저");
  assert.equal(ui.type, "grammar");
  assert.equal(ui.category, "助词");
  assert.equal(ui.discipline, "kr-grammar");
  assert.equal(ui.level.code, "");
  assert.equal(ui.level.rank, 0);
  assert.deepEqual(ui.tags, ["助词", "强调表达"]);
  assert.equal(ui.summary, items[0].summary);
  assert.deepEqual(ui.aliases, []);
  assert.deepEqual(ui.related, []);
  assert.equal(ui.content, null);
  assert.equal(ui.kind, "grammar");
});

test("toEntryItem 映射真实 -는데（TOPIK II：level 'II' rank 2）", () => {
  const { items } = loadFixture("entry-list.sample.json");
  const ui = toEntryItem(items[1]);
  assert.equal(ui.uid, items[1].uid);
  assert.equal(ui.name, "-는데");
  assert.equal(ui.category, "连接语尾");
  assert.equal(ui.level.code, "II");
  assert.equal(ui.level.rank, 2);
  assert.deepEqual(ui.tags, ["连接语尾"]);
  assert.equal(ui.summary, items[1].summary);
});

test("toSetShape 把 SubjectMeta 归一为 UI set 形状（id/discipline=kr-grammar + counts）", () => {
  const meta = loadFixture("subject-meta.sample.json");
  const set = toSetShape(meta);
  assert.equal(set.id, "kr-grammar");
  assert.equal(set.discipline, "kr-grammar");
  assert.equal(set.name, "韩语语法");
  assert.equal(set.color, "#D97706");
  assert.ok(set.levels.find((l) => l.code === "I" && l.label === "TOPIK I" && l.rank === 1));
  assert.ok(set.levels.find((l) => l.code === "II" && l.label === "TOPIK II" && l.rank === 2));
  assert.equal(set.types[0].value, "grammar");
  assert.equal(set.counts.total, 642);
  assert.equal(set.counts.by_level["II"], 170);
});

test("toDetailView 透传 content（fields/lists/related 键与模板字段对齐），images 规范化为 []", () => {
  const payload = loadFixture("detail-payload.sample.json");
  const view = toDetailView(payload);
  // 模板 fields 的所有 key 都应在 view.fields 中可读（content 透传保证）
  const template = loadRealTemplate();
  const fieldSection = template.sections.find((s) => s.type === "fields");
  for (const f of fieldSection.fields) {
    assert.ok(f.key in view.fields, `view.fields 缺少模板 key: ${f.key}`);
  }
  // content 原样透传
  assert.equal(view.fields.pattern, "-기 때문에");
  assert.equal(view.fields.form_rule, "动词词干 + -기 때문에");
  assert.deepEqual(view.fields.variants, ["-기 때문이에요"]);
  assert.deepEqual(view.fields.attaches_to, ["动词", "形容词"]);
  assert.equal(view.fields.usage_scene, "书面与口语均常用，强调原因时可用。");
  // lists / related
  assert.equal(view.lists.meanings.length, 1);
  assert.equal(view.lists.usages.length, 1);
  assert.equal(view.lists.examples.length, 2);
  assert.equal(view.lists.cautions.length, 1);
  assert.equal(view.lists.collocations.length, 1);
  assert.equal(view.related.resolved.length, 1);
  assert.equal(view.related.resolved[0].headword, "-느라고");
  assert.equal(view.related.pending.length, 2);
  assert.equal(view.related.pending[0].targetText, "-아/어서");
  // images：无图学科 → []
  assert.deepEqual(view.images, []);
});

test("toDetailView 容错：content/images 缺失不抛错，返回空壳", () => {
  assert.deepEqual(toDetailView(null), { images: [] });
  assert.deepEqual(toDetailView({ content: null, images: undefined }), { images: [] });
  assert.deepEqual(toDetailView({ content: { fields: {} }, images: null }), {
    fields: {},
    images: [],
  });
});

test("exportGrammarMarkdown 用模板 label 生成全部有值节、隐藏空节、包含未收录折叠", () => {
  const payload = loadFixture("detail-payload.sample.json");
  const view = toDetailView(payload);
  const template = loadRealTemplate();
  const setMeta = loadFixture("subject-meta.sample.json");
  const set = { ...toSetShape(setMeta), template };
  const entry = {
    name: payload.headword,
    summary: payload.summary,
    category: payload.category,
    level: { code: payload.level_code },
  };

  const md = exportGrammarMarkdown(entry, set, view);
  assert.ok(md.includes("# -기 때문에"), "应包含标题");
  assert.ok(md.includes("TOPIK II"), "应包含等级（set.levels label）");
  assert.ok(md.includes("因为……，由于……"), "顶部摘要应为 entries.summary");
  assert.ok(md.includes("## 语法信息"), "应包含 fields 节标题");
  assert.ok(md.includes("## 意思"), "应包含 meanings 列表标题");
  assert.ok(md.includes("## 使用方法"), "应包含 usages 列表标题");
  assert.ok(md.includes("## 使用场景"), "应包含 scenes 列表标题");
  assert.ok(md.includes("## 例句（双语）"), "应包含 examples 标题");
  assert.ok(md.includes("비가 오기 때문에"), "应包含例句韩文");
  assert.ok(md.includes("## 注意事项"), "应包含 cautions 标题");
  assert.ok(md.includes("## 常见搭配"), "应包含 collocations 标题");
  assert.ok(md.includes("近似语法"), "应包含 related 标题");
  assert.ok(md.includes("-느라고"), "应包含已解析关联");
  assert.ok(md.includes("另有 2 条未收录关联"), "应包含未收录折叠计数");
  assert.ok(md.includes("-아/어서"), "应包含未收录原文");

  // 空子表隐藏：把 lists 清空后对应节不再出现
  const emptyLists = Object.fromEntries(
    Object.keys(view.lists).map((k) => [k, []]),
  );
  const emptyMd = exportGrammarMarkdown(entry, set, { ...view, lists: emptyLists });
  assert.ok(!emptyMd.includes("## 意思"), "空 meanings 子表应整节隐藏");
  assert.ok(!emptyMd.includes("## 使用场景"), "空 scenes 子表应整节隐藏");
  assert.ok(!emptyMd.includes("## 常见搭配"), "空 collocations 子表应整节隐藏");
});
