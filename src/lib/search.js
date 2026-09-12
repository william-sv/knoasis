// Knoasis UI 原型 · 客户端搜索
// 模拟 v1.3 三路由搜索的“前端等价物”：
// 检索字段 = 名称 / 别名 / 摘要 / 标签 / 类型 / 学科；命中按权重简单打分。
// 中文按子串匹配即可（条目量小），无需 FTS；将来由 Rust search 命令替换。

const QUERY_FIELDS = ["name", "summary", "aliases", "tags", "type", "category"];

// 归一化：去首尾空白 + 小写 + 全角转半角（搜索体验贴近真实产品）
export function normalizeQuery(q) {
  let s = String(q ?? "").trim().toLowerCase();
  const map = {
    "　": " ",
    "！": "!",
    "？": "?",
    "，": ",",
    "。": ".",
    "（": "(",
    "）": ")",
    "：": ":",
    "；": ";",
  };
  s = s.replace(/[！？，。：；（）]/g, (ch) => map[ch] ?? ch);
  return s;
}

function fieldString(entry, field) {
  const v = entry[field];
  if (v == null) return "";
  if (Array.isArray(v)) return v.join(" ");
  return String(v);
}

function searchIn(entry, q, field, weight) {
  const text = normalizeQuery(fieldString(entry, field));
  if (!text) return 0;
  // 数组字段里某个元素精确命中给更高分
  const raw = entry[field];
  if (Array.isArray(raw)) {
    for (const item of raw) {
      if (normalizeQuery(item) === q) return weight * 3;
    }
  }
  if (text.includes(q)) return weight;
  return 0;
}

/**
 * 在给定条目范围内执行搜索
 * @param {Array} scopeEntries 候选条目（当前学科 / 全部）
 * @param {string} rawQuery    用户原始输入
 * @returns {Array} 按分数降序的条目数组
 */
export function searchEntries(scopeEntries, rawQuery) {
  const q = normalizeQuery(rawQuery);
  if (!q) return [];

  const scored = [];
  for (const entry of scopeEntries) {
    let score = 0;

    // 名称：精确 > 前缀 > 子串（英文名大小写不敏感）
    const name = normalizeQuery(entry.name);
    const nameLower = name.toLowerCase();
    const qLower = q.toLowerCase();
    if (nameLower === qLower) score += 1000;
    else if (nameLower.startsWith(qLower)) score += 500;
    else if (nameLower.includes(qLower)) score += 200;

    // 别名：元素级权重更高
    score += searchIn(entry, q, "aliases", 120);

    // 摘要子串
    score += searchIn(entry, q, "summary", 30);

    // 标签
    score += searchIn(entry, q, "tags", 20);

    // 类型 / 学科 / 分类（中文关键词兜底，如“语法”“定理”）
    score += searchIn(entry, q, "type", 12);
    score += searchIn(entry, q, "category", 10);
    if (entry.discipline === q) score += 40;

    if (score > 0) {
      scored.push({ entry, score });
    }
  }

  scored.sort((a, b) => {
    if (b.score !== a.score) return b.score - a.score;
    return a.entry.name.length - b.entry.name.length;
  });
  return scored.map((s) => s.entry);
}
