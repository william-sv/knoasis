// Knoasis · 条目详情 → Markdown 导出（模板驱动，学科包通用）
//
// 教师刚需：把模板驱动渲染的详情复制为可粘贴的 Markdown。
// 文案结构：# 条目名 / 学科 · 等级 · 分类 → 逐模板 sections：
//  - fields：字段表（有值才导出）
//  - list：子表（空隐藏；key=related 走相关知识点 + 未收录折叠；senses 等走通用 itemFields）
//  - paragraph：整段全文（pre-wrap 保留换行）
//  - images：导出占位「（共 N 张图解，见应用内）」（图片无法进剪贴板文本）
//  - connections_grouped：按 动词/形容词/名词 分组导出（接续 + 活用 + 实例 + 语义 + 时态 + 限制）
//  - examples：例句表（ko — zh ｜ note）
//  - common_errors：易错点（~~wrong~~ → right ｜ note）
//  - similar：近似语法（headword：difference）
//  - related：相关语法字符串数组（内部 uid 经 resolveRelated 解析为 headword；ext: 去前缀）
// 不再假设任何学科特有键。

function fmtValue(v) {
  if (v == null) return "";
  if (Array.isArray(v)) return v.filter((x) => String(x ?? "").trim()).join("、");
  return String(v);
}

function isBlank(v) {
  if (v == null) return true;
  if (Array.isArray(v)) return v.every((x) => String(x ?? "").trim() === "");
  return String(v).trim() === "";
}

/** 等级 label：取 set.levels 下发的 label（学科包 meta），找不到回退 code 本身。 */
function levelLabelOfEntry(entry, set) {
  const code = entry && entry.level ? entry.level.code : "";
  if (set && Array.isArray(set.levels)) {
    const lv = set.levels.find((l) => l.code === code);
    if (lv && lv.label) return lv.label;
  }
  return code || "未分级";
}

const ATTACH_LABELS = { verb: "动词", adjective: "形容词", noun: "名词" };
const ATTACH_ORDER = ["verb", "adjective", "noun"];
const TENSE_LABELS = { past: "过去时", future: "将来时" };

/** connections_grouped：按 attachesTo 分组导出 */
function pushConnectionsGrouped(view, label, L) {
  const conns = Array.isArray(view?.connections) ? view.connections : [];
  if (!conns.length) return;
  const map = new Map();
  for (const c of conns) {
    const k = c && c.attachesTo ? String(c.attachesTo) : "other";
    if (!map.has(k)) map.set(k, []);
    map.get(k).push(c);
  }
  const order = [
    ...ATTACH_ORDER.filter((k) => map.has(k)),
    ...[...map.keys()].filter((k) => !ATTACH_ORDER.includes(k)),
  ];
  L.push("", `## ${label}`);
  for (const k of order) {
    const items = map.get(k);
    L.push("", `### ${ATTACH_LABELS[k] || k}（${items.length}）`);
    for (const c of items) {
      if (!isBlank(c.requiredForm)) L.push(`- **接续**：\`${c.requiredForm}\``);
      const reals = Array.isArray(c.realizations) ? c.realizations : [];
      if (reals.length) {
        const parts = reals.map((r) => {
          let s = `${r.stem} → ${r.ko}`;
          if (!isBlank(r.irregularity)) s += `（${r.irregularity}）`;
          if (!isBlank(r.note)) s += `[${r.note}]`;
          return s;
        });
        L.push(`  - 活用：${parts.join("；")}`);
      }
      if (!isBlank(c.example)) L.push(`  - 实例：${c.example}`);
      if (!isBlank(c.meaning)) L.push(`  - 语义：${c.meaning}`);
      if (!isBlank(c.tense)) L.push(`  - 时态：${TENSE_LABELS[c.tense] || c.tense}`);
      const cs = c.constraints;
      if (cs && typeof cs === "object") {
        if (cs.sequential && cs.sequential.sameSubject) L.push("  - 限制：顺序义前后分句主语须一致");
        if (cs.causal && Array.isArray(cs.causal.excludeMood) && cs.causal.excludeMood.length) {
          L.push(`  - 限制：因果义禁用句末语气（${cs.causal.excludeMood.join(" / ")}）`);
        }
        if (Array.isArray(cs.incompatibleWith) && cs.incompatibleWith.length) {
          L.push(`  - 限制：互斥形态（${cs.incompatibleWith.join(" / ")}）`);
        }
      }
    }
  }
}

/** related（v11 字符串数组）：内部 uid 解析为 headword；ext: 去前缀 */
function pushRelated(view, label, resolveRelated, L) {
  const arr = Array.isArray(view?.related) ? view.related : [];
  if (!arr.length) return;
  const lines = [];
  for (const raw of arr) {
    const s = String(raw ?? "").trim();
    if (!s) continue;
    if (s.startsWith("ext:")) {
      lines.push(`- ${s.slice(4)}（外部概念）`);
    } else if (s.startsWith("kr-grammar:")) {
      const name = typeof resolveRelated === "function" ? resolveRelated(s) : null;
      lines.push(`- ${name || s}`);
    } else {
      lines.push(`- ${s}`);
    }
  }
  if (lines.length) L.push("", `## ${label}`, ...lines);
}

/** examples（type=examples）：view.lists.examples */
function pushExamples(view, label, L) {
  const rows = Array.isArray(view?.lists?.examples) ? view.lists.examples : [];
  const lines = [];
  for (const e of rows) {
    if (isBlank(e.ko)) continue;
    let line = `- ${e.ko}`;
    if (!isBlank(e.zh)) line += ` — ${e.zh}`;
    if (!isBlank(e.note)) line += ` ｜ ${e.note}`;
    lines.push(line);
  }
  if (lines.length) L.push("", `## ${label}`, ...lines);
}

/** common_errors（type=common_errors） */
function pushErrors(view, label, L) {
  const rows = Array.isArray(view?.common_errors) ? view.common_errors : [];
  const lines = [];
  for (const r of rows) {
    const bits = [];
    if (!isBlank(r.wrong)) bits.push(`~~${r.wrong}~~`);
    if (!isBlank(r.right)) bits.push(r.right);
    if (!bits.length) continue;
    let line = `- ${bits.join(" → ")}`;
    if (!isBlank(r.note)) line += ` ｜ ${r.note}`;
    lines.push(line);
  }
  if (lines.length) L.push("", `## ${label}`, ...lines);
}

/** similar / antonyms（headword + difference 型）：无值整节隐藏 */
function pushHeadwordDiff(view, label, key, L) {
  const rows = Array.isArray(view?.[key]) ? view[key] : [];
  const lines = [];
  for (const r of rows) {
    if (isBlank(r.headword) && isBlank(r.difference)) continue;
    lines.push(`- **${r.headword || ""}**：${r.difference || ""}`);
  }
  if (lines.length) L.push("", `## ${label}`, ...lines);
}

/**
 * 生成条目详情 Markdown（任意学科包通用）。
 * @param {object} entry   UI 条目（含 name/category/level.code/summary）
 * @param {object|null} set 学科 set（取 levels label / template；可为空）
 * @param {object} view    adapter.toDetailView(payload)（content 透传后的模板数据对象）
 * @param {(uid:string)=>string|null} [resolveRelated] related 内部 uid → headword 解析器
 * @returns {string}
 */
export function exportGrammarMarkdown(entry, set, view, resolveRelated) {
  const L = [];
  L.push(`# ${entry.name || ""}`);

  const metaBits = [];
  if (set) metaBits.push(set.name);
  metaBits.push(levelLabelOfEntry(entry, set));
  if (entry.category && entry.category !== "未分类") metaBits.push(entry.category);
  if (metaBits.length) L.push(metaBits.join(" · "));

  // 顶部摘要（学科包 entries.summary 预览；无则不导）
  if (entry.summary) L.push("", entry.summary);

  if (!set || !Array.isArray(set.template?.sections)) {
    L.push("", "（详情模板缺失，仅导出条目元信息）");
    return L.join("\n");
  }

  for (const section of set.template.sections) {
    const key = section.key;
    const label = section.label || key;

    // paragraph：整段讲解（OCR 原样，标题 + 全文）
    if (section.type === "paragraph") {
      const text = fmtValue(view?.paragraphs?.[section.fieldKey || section.key]);
      if (!isBlank(text)) L.push("", `## ${label}`, text);
      continue;
    }

    // images：图解只能应用内看，导出占位
    if (section.type === "images") {
      const imgs = Array.isArray(view?.images) ? view.images : [];
      if (imgs.length) L.push("", `## ${label}`, `（共 ${imgs.length} 张图解，见应用内）`);
      continue;
    }

    if (section.type === "fields") {
      const rows = [];
      for (const f of section.fields || []) {
        const v = fmtValue(view?.fields?.[f.key]);
        if (isBlank(v)) continue;
        rows.push(`- **${f.label || f.key}**：${v}`);
      }
      if (rows.length) L.push("", `## ${label}`, ...rows);
      continue;
    }

    if (section.type === "connections_grouped") {
      pushConnectionsGrouped(view, label, L);
      continue;
    }
    if (section.type === "examples") {
      pushExamples(view, label, L);
      continue;
    }
    if (section.type === "common_errors") {
      pushErrors(view, label, L);
      continue;
    }
    if (section.type === "similar" || section.type === "antonyms") {
      pushHeadwordDiff(view, label, key, L);
      continue;
    }
    if (section.type === "related") {
      pushRelated(view, label, resolveRelated, L);
      continue;
    }

    if (section.type === "list") {
      if (key === "related" && !Array.isArray(view?.related)) {
        // 旧 {resolved,pending} 结构（兼容）
        const rel = view?.related || { resolved: [], pending: [] };
        const lines = [];
        for (const r of rel.resolved || []) {
          const relBit = r.relation ? `（${r.relation}）` : "";
          const noteBit = r.note ? ` — ${r.note}` : "";
          lines.push(`- ${r.headword}${relBit}${noteBit}`);
        }
        const pend = rel.pending || [];
        if (pend.length) {
          lines.push(`- 另有 ${pend.length} 条未收录关联：`);
          for (const p of pend.slice(0, 20)) {
            lines.push(`  - ${p.targetText}${p.relation ? `（${p.relation}）` : ""}`);
          }
          if (pend.length > 20) lines.push(`  - … 其余 ${pend.length - 20} 条省略`);
        }
        if (lines.length) L.push("", `## ${label}`, ...lines);
        continue;
      }
      const items = (view?.lists?.[key] || [])
        .map((it) => {
          const segs = [];
          for (const f of section.itemFields || []) {
            const v = fmtValue(it?.[f.key]);
            if (isBlank(v)) continue;
            segs.push(`**${f.label || f.key}**：${v}`);
          }
          return segs.length ? `- ${segs.join(" ｜ ")}` : null;
        })
        .filter(Boolean);
      if (items.length) L.push("", `## ${label}`, ...items);
    }
  }

  return L.join("\n");
}
