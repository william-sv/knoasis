// Knoasis · 条目详情 → Markdown 导出（模板驱动，学科包通用）
//
// 教师刚需：把模板驱动渲染的详情复制为可粘贴的 Markdown。
// 文案结构：# 条目名 / 学科 · 等级 · 分类 → 逐模板 sections：
//  - fields：字段表（有值才导出）
//  - list：子表（空隐藏；key=related 走相关知识点 + 未收录折叠）
//  - paragraph：整段全文（pre-wrap 保留换行）
//  - images：导出占位「（共 N 张图解，见应用内）」（图片无法进剪贴板文本）
// 不再假设任何学科特有键（如 kr 的 meaning_brief/usage_scene/en 的 explanation）。

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

/**
 * 生成条目详情 Markdown（任意学科包通用）。
 * @param {object} entry   UI 条目（含 name/category/level.code/summary）
 * @param {object|null} set 学科 set（取 levels label / template；可为空）
 * @param {object} view    adapter.toDetailView(payload)（content 透传后的模板数据对象）
 * @returns {string}
 */
export function exportGrammarMarkdown(entry, set, view) {
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

    if (section.type === "list") {
      if (key === "related") {
        // related 单独处理（相关知识点 + 未收录折叠）
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
      const items = (view?.lists?.[key] || []).map((it) => {
        const segs = [];
        for (const f of section.itemFields || []) {
          const v = fmtValue(it?.[f.key]);
          if (isBlank(v)) continue;
          segs.push(`**${f.label || f.key}**：${v}`);
        }
        return segs.length ? `- ${segs.join(" ｜ ")}` : null;
      }).filter(Boolean);
      if (items.length) L.push("", `## ${label}`, ...items);
    }
  }

  return L.join("\n");
}
