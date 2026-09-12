// 沙箱验证：通用 content 透传（paragraphs/images 型学科包）+ 通用导出
// 对齐 docs/Knoasis-学科数据组织与第三方接入方案.md §3.6：任何学科包 content 均为
// view-ready JSON；adapter 只透传 content 并把 payload.images（已按 image_hidden 过滤的
// 绝对路径）规范化为 view.images。本文件用「段落 + 图解」型集合验证通用路径，不再有 en 专属分支。
// 运行：node --test src/lib/grammar/__tests__/detail-passthrough.test.js
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

import { toSetShape, toDetailView, toEntryItem } from "../adapter.js";
import { exportGrammarMarkdown } from "../export.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "../../../..");

function loadFixture(name) {
  return JSON.parse(
    readFileSync(path.join(__dirname, "../fixtures", name), "utf8"),
  );
}

function loadParaImagesTemplate() {
  const raw = readFileSync(
    path.join(repoRoot, "src-tauri/resources/grammar/template_registry.json"),
    "utf8",
  );
  return JSON.parse(raw).en_grammar;
}

test("toDetailView 透传 paragraphs/lists/related 且 payload.images（绝对路径）覆盖 content 内 rel 列表", () => {
  const payload = {
    uid: "en-grammar:60303ae22b99",
    set_id: "en-grammar",
    headword: "定语从句",
    category: "定语从句",
    level_code: "III",
    level_label: "高级",
    tags: ["句法"],
    summary: "定语从句用于修饰名词或代词。",
    content: {
      fields: { summary: "定语从句用于修饰名词或代词。" },
      paragraphs: { explanation: "关系副词（when / where / why）引导定语从句。" },
      lists: {
        examples: [
          { en: "The book that you lent me is very helpful.", zh: "你借给我的那本书很有帮助。", note: "" },
        ],
      },
      // 包内 content 允许存 rel 形式；渲染一律以 payload.images（解析+过滤后）为准
      images: [{ rel: "book/old.jpeg" }],
      related: { resolved: [], pending: [] },
    },
    images: [
      { rel: "book/dingyu01.jpeg", path: "/abs/path/book/dingyu01.jpeg" },
      { rel: "book/dingyu02.jpeg", path: "/abs/path/book/dingyu02.jpeg" },
    ],
  };
  const view = toDetailView(payload);
  assert.equal(view.paragraphs.explanation, payload.content.paragraphs.explanation);
  assert.equal(view.fields.summary, payload.content.fields.summary);
  assert.equal(view.lists.examples[0].en, "The book that you lent me is very helpful.");
  assert.deepEqual(view.related, { resolved: [], pending: [] });
  // images 覆盖为绝对路径列表（被隐藏/缺失项由 Rust 过滤，前端只拿到可见图）
  assert.deepEqual(view.images, payload.images);
});

test("toSetShape 通用学科 meta：id=set_id、levels label 由包下发（非 TOPIK）", () => {
  const meta = loadFixture("subject-meta-en.sample.json");
  const set = toSetShape(meta);
  assert.equal(set.id, "en-grammar");
  assert.equal(set.discipline, "en-grammar");
  assert.equal(set.name, "英语语法");
  assert.equal(set.color, "#2563EB");
  assert.equal(set.levelSystemLabel, "难度");
  assert.ok(set.levels.find((l) => l.code === "I" && l.label === "基础" && l.rank === 1));
  assert.ok(set.levels.find((l) => l.code === "II" && l.label === "进阶" && l.rank === 2));
  assert.ok(set.levels.find((l) => l.code === "III" && l.label === "高级" && l.rank === 3));
});

test("exportGrammarMarkdown 通用：paragraph 全文 / images 占位 / list 例句，无学科特有键", () => {
  const template = loadParaImagesTemplate();
  const view = toDetailView({
    uid: "en-grammar:60303ae22b99",
    set_id: "en-grammar",
    headword: "定语从句",
    category: "定语从句",
    level_code: "III",
    level_label: "高级",
    tags: [],
    summary: "定语从句用于修饰名词或代词。",
    content: {
      fields: {},
      paragraphs: { explanation: "关系副词（when / where / why）引导定语从句。" },
      lists: {
        examples: [
          { en: "The book that you lent me is very helpful.", zh: "你借给我的那本书很有帮助。", note: "that 作宾语，可省略" },
        ],
      },
      related: { resolved: [], pending: [] },
    },
    images: [
      { rel: "book/a.jpeg", path: "/abs/a.jpeg" },
      { rel: "book/b.jpeg", path: "/abs/b.jpeg" },
    ],
  });
  const set = {
    name: "英语语法",
    levels: [
      { code: "I", label: "基础", rank: 1 },
      { code: "II", label: "进阶", rank: 2 },
      { code: "III", label: "高级", rank: 3 },
    ],
    template,
  };
  const entry = {
    name: "定语从句",
    category: "定语从句",
    level: { code: "III" },
  };

  const md = exportGrammarMarkdown(entry, set, view);
  assert.ok(md.includes("# 定语从句"), "应包含标题");
  assert.ok(md.includes("英语语法 · 高级 · 定语从句"), "meta 行取 set.levels label");
  assert.ok(!md.includes("TOPIK"), "不应出现 TOPIK 字样");
  assert.ok(md.includes("## 讲解"), "应包含讲解节标题");
  assert.ok(md.includes("关系副词（when / where / why）"), "paragraph 应含全文");
  assert.ok(md.includes("## 图解"), "应包含图解节标题");
  assert.ok(md.includes("（共 2 张图解，见应用内）"), "images 应导出占位文案");
  assert.ok(md.includes("## 例句"), "应包含例句节标题");
  assert.ok(md.includes("The book that you lent me is very helpful."), "应包含例句");
  assert.ok(!md.includes("## 概述"), "不应出现 kr 专属概述");
});

test("toEntryItem 支持 discipline 参数（多学科通用）；未传默认空串；III → rank 3", () => {
  const item = {
    uid: "en-grammar:60303ae22b99",
    headword: "定语从句",
    category: "句法",
    level_code: "III",
    level_label: "高级",
    tags: ["句法"],
    summary: "",
  };
  const ui = toEntryItem(item, "en-grammar");
  assert.equal(ui.discipline, "en-grammar");
  assert.equal(ui.name, "定语从句");
  assert.equal(ui.level.code, "III");
  assert.equal(ui.level.rank, 3);
  const fallback = toEntryItem(item);
  assert.equal(fallback.discipline, "");
});

test("exportGrammarMarkdown 模板缺失时仅导出条目元信息（不抛错）", () => {
  const entry = { name: "N마저", summary: "连…都", category: "补助词", level: { code: "II" } };
  const set = {
    name: "韩语语法",
    levels: [
      { code: "I", label: "TOPIK I", rank: 1 },
      { code: "II", label: "TOPIK II", rank: 2 },
    ],
    template: null,
  };
  const md = exportGrammarMarkdown(entry, set, null);
  assert.ok(md.includes("# N마저"));
  assert.ok(md.includes("韩语语法 · TOPIK II · 补助词"));
  assert.ok(md.includes("连…都"));
  assert.ok(md.includes("（详情模板缺失，仅导出条目元信息）"));
});
