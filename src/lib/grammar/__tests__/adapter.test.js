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
  return JSON.parse(raw).ko_grammar;
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
  assert.equal(ui.discipline, "");
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

test("toDetailView 透传 v11 content（条目级字段并入 + paragraphs/connections/lists/similar/common_errors/related 字符串数组），images 规范化为 []", () => {
  const payload = {
    uid: "kr-grammar:aaaa11112222",
    set_id: "kr-grammar",
    headword: "아/어/여서",
    category: "连接语尾",
    level_code: "I",
    level_label: "TOPIK I",
    tags: ["原因(이유)"],
    summary: "因为……所以……",
    content: {
      // 条目级字段由导入器并入 content（Rust get_entry 不返回这些）
      id: "korozc5ni4cgc2",
      type: "어미",
      pos: "어미",
      speechLevel: "",
      aliases: ["-아/어/여서"],
      sources: [{ type: "nikl", ref: "표준국어대사전" }],
      related: ["kr-grammar:0c32ab5b7b4e", "ext:정말"],
      paragraphs: { explanation: "元音ㅏ/ㅗ 后接 -아서。" },
      connections: [
        {
          attachesTo: "verb",
          requiredForm: "동사 어간 + 아/어/여서",
          example: "가서",
          realizations: [{ stem: "가다", ko: "가서" }],
        },
      ],
      lists: {
        examples: [{ ko: "가서", zh: "去然后", note: "", audio: "" }],
        senses: [{ sense: "顺序", usage: "…" }],
      },
      similar: [{ headword: "-고", difference: "…" }],
      antonyms: [],
      common_errors: [{ wrong: "w", right: "r", note: "n" }],
      images: [],
    },
    images: [],
  };
  const view = toDetailView(payload);
  // 条目级字段透传
  assert.equal(view.id, "korozc5ni4cgc2");
  assert.equal(view.pos, "어미");
  assert.deepEqual(view.aliases, ["-아/어/여서"]);
  assert.deepEqual(view.sources, [{ type: "nikl", ref: "표준국어대사전" }]);
  assert.deepEqual(view.related, ["kr-grammar:0c32ab5b7b4e", "ext:정말"]);
  // 教学主体透传
  assert.equal(view.paragraphs.explanation, "元音ㅏ/ㅗ 后接 -아서。");
  assert.equal(view.connections[0].attachesTo, "verb");
  assert.equal(view.lists.examples.length, 1);
  assert.equal(view.lists.senses.length, 1);
  assert.equal(view.similar[0].headword, "-고");
  assert.equal(view.common_errors[0].right, "r");
  // 模板 sections 的取数键均可在 view 中命中（契约对齐）
  const template = loadRealTemplate();
  const fetch = (s) => {
    if (s.type === "meta") return true;
    if (s.type === "paragraph") return (view.paragraphs || {})[s.fieldKey || s.key];
    if (s.type === "connections_grouped") return view.connections;
    if (s.type === "examples" || s.type === "list") return (view.lists || {})[s.key];
    if (s.type === "related") return view.related;
    return view[s.key];
  };
  for (const s of template.sections) {
    assert.ok(fetch(s) !== undefined, `view 缺少模板取数键: ${s.type}.${s.key}`);
  }
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

test("exportGrammarMarkdown 用 v11 模板 label 生成全部有值节、隐藏空节、related 字符串解析", () => {
  const template = loadRealTemplate();
  const view = {
    id: "korozc5ni4cgc2",
    type: "어미",
    pos: "어미",
    aliases: ["-아/어/여서"],
    sources: [{ type: "nikl", ref: "표준국어대사전" }],
    related: ["kr-grammar:0c32ab5b7b4e", "ext:정말"],
    paragraphs: { explanation: "完整讲解文本。" },
    connections: [
      {
        attachesTo: "verb",
        requiredForm: "동사 어간 + 아/어/여서",
        example: "가서",
        realizations: [
          { stem: "가다", ko: "가서" },
          { stem: "하다", ko: "해서", irregularity: "ha-irr" },
        ],
        meaning: "表顺序",
      },
      { attachesTo: "noun", requiredForm: "N + 이어서", example: "학생이어서", realizations: [] },
    ],
    lists: {
      examples: [{ ko: "비가 와서", zh: "因为下雨", note: "" }],
      senses: [{ sense: "顺序", usage: "用法说明" }],
    },
    similar: [{ headword: "-고", difference: "辨析文本" }],
    antonyms: [],
    common_errors: [{ wrong: "못 잤어서", right: "못 자서", note: "不能与过去时连用" }],
    images: [],
  };
  const set = {
    name: "韩语语法",
    levels: [
      { code: "I", label: "TOPIK I", rank: 1 },
      { code: "II", label: "TOPIK II", rank: 2 },
    ],
    template,
  };
  const entry = {
    name: "아/어/여서",
    summary: "因为……所以……",
    category: "连接语尾",
    level: { code: "I" },
  };
  const resolveRelated = (uid) => (uid === "kr-grammar:0c32ab5b7b4e" ? "-고" : null);

  const md = exportGrammarMarkdown(entry, set, view, resolveRelated);
  assert.ok(md.includes("# 아/어/여서"), "应包含标题");
  assert.ok(md.includes("TOPIK I"), "应包含等级（set.levels label）");
  assert.ok(md.includes("## 语法释义"), "应包含讲解节标题");
  assert.ok(md.includes("完整讲解文本。"), "paragraph 应含全文");
  assert.ok(md.includes("## 语法用法详解（活用 / 接续规则）"), "应包含用法详解标题");
  assert.ok(md.includes("### 动词（1）"), "应按 attachesTo 分动词块");
  assert.ok(md.includes("### 名词（1）"), "应按 attachesTo 分名词块");
  assert.ok(md.includes("동사 어간 + 아/어/여서"), "应含接续公式");
  assert.ok(md.includes("가다 → 가서"), "应含形态实现");
  assert.ok(md.includes("## 例句"), "应包含例句节标题");
  assert.ok(md.includes("비가 와서"), "应包含例句韩文");
  assert.ok(md.includes("## 易错点"), "应包含易错点标题");
  assert.ok(md.includes("~~못 잤어서~~"), "易错点误用应带删除线");
  assert.ok(md.includes("## 近似语法"), "应包含近似语法标题");
  assert.ok(md.includes("-고"), "应包含近似语法条目");
  assert.ok(!md.includes("## 相反语法"), "无 antonyms 数据时「相反语法」模块应整节隐藏");
  assert.ok(md.includes("## 义项"), "应包含义项标题");
  assert.ok(md.includes("顺序"), "应包含义项内容");
  assert.ok(md.includes("## 相关语法"), "应包含相关语法标题");
  assert.ok(md.includes("정말（外部概念）"), "外部 ext: 应去前缀展示");

  // 空子表整节隐藏
  const emptyView = {
    ...view,
    paragraphs: {},
    connections: [],
    lists: { examples: [], senses: [] },
    similar: [],
    common_errors: [],
    related: [],
  };
  const emptyMd = exportGrammarMarkdown(entry, set, emptyView, () => null);
  assert.ok(!emptyMd.includes("## 语法释义"), "空 paragraphs 应整节隐藏");
  assert.ok(!emptyMd.includes("## 语法用法详解"), "空 connections 应整节隐藏");
  assert.ok(!emptyMd.includes("## 例句"), "空 examples 应整节隐藏");
  assert.ok(!emptyMd.includes("## 易错点"), "空 common_errors 应整节隐藏");
  assert.ok(!emptyMd.includes("## 近似语法"), "空 similar 应整节隐藏");
  assert.ok(!emptyMd.includes("## 相关语法"), "空 related 应整节隐藏");
  assert.ok(!emptyMd.includes("## 义项"), "空 senses 应整节隐藏");
});
