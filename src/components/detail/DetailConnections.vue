<script setup>
// 详情 · 语法用法详解（活用 / 接续规则）：按 attachesTo 分 动词 / 形容词 / 名词 三块
// 每块内逐条 connection 展示：接续公式 + 形态实现(realizations) + 实例 + 语义 + 时态 + 选择限制
// 数据来源：v11 content.connections（每个 connection 自带 attachesTo，权威键）
import { computed } from "vue";

const props = defineProps({
  label: { type: String, default: "" },
  connections: { type: Array, default: () => [] },
});

const ATTACH_ORDER = ["verb", "adjective", "noun"];
const ATTACH_LABELS = { verb: "动词", adjective: "形容词", noun: "名词" };
const TENSE_LABELS = { past: "过去时", future: "将来时" };
const IRREG_LABELS = {
  "b-irr": "ㅂ 不规则",
  "h-irr": "ㅎ 不规则",
  contraction: "缩合",
  "d-irr": "ㄷ 不规则",
  "s-irr": "ㅅ 不规则",
  "eu-drop": "ㅡ 脱落",
  "ha-irr": "하 不规则",
  "r-irr": "르 不规则",
  "l-irr": "ㄹ 不规则",
};

function isBlank(v) {
  if (v == null) return true;
  if (Array.isArray(v)) return v.length === 0;
  return String(v).trim() === "";
}

// 归组：verb → adjective → noun（仅保留有数据的组），其它未知键附在末尾
const groups = computed(() => {
  const map = new Map();
  for (const c of props.connections || []) {
    const key = c && c.attachesTo ? String(c.attachesTo) : "other";
    if (!map.has(key)) map.set(key, []);
    map.get(key).push(c);
  }
  const out = [];
  for (const k of ATTACH_ORDER) {
    if (map.has(k)) out.push({ key: k, label: ATTACH_LABELS[k], items: map.get(k) });
  }
  for (const [k, items] of map.entries()) {
    if (!ATTACH_ORDER.includes(k)) out.push({ key: k, label: k, items });
  }
  return out;
});

function realsOf(c) {
  return Array.isArray(c && c.realizations) ? c.realizations : [];
}
function irregLabel(code) {
  if (!code) return "";
  // 源数据 irregularity 已为韩文受控标签（하不规则 等）；旧缩写码亦兼容
  return IRREG_LABELS[code] || code;
}
// 备注文案：irregularity（모음축약 / 하不规则…）+ note，两者都有则以「·」连接
function remarkOf(r) {
  const parts = [];
  const irr = irregLabel(r && r.irregularity);
  if (irr) parts.push(irr);
  if (!isBlank(r && r.note)) parts.push(String(r.note).trim());
  return parts.join(" · ");
}

// 该 connection 是否需要「备注」列（任一 realization 有 irregularity 或 note）
function hasRemarkOf(c) {
  return realsOf(c).some((r) => remarkOf(r) !== "");
}

// constraints → 可读文本行（稀疏，仅 2 条 connection 有）
function constraintLines(c) {
  const cs = c && c.constraints;
  if (!cs || typeof cs !== "object") return [];
  const lines = [];
  if (cs.sequential && cs.sequential.sameSubject) lines.push("顺序义：前后分句主语须一致");
  if (cs.causal && Array.isArray(cs.causal.excludeMood) && cs.causal.excludeMood.length) {
    lines.push(`因果义：禁用句末语气（${cs.causal.excludeMood.join(" / ")}）`);
  }
  if (Array.isArray(cs.incompatibleWith) && cs.incompatibleWith.length) {
    lines.push(`互斥形态：${cs.incompatibleWith.join(" / ")}`);
  }
  return lines;
}
</script>

<template>
  <section v-if="groups.length" class="cxs">
    <h2 v-if="label" class="cxs-title">{{ label }}</h2>

    <div class="cxs-groups">
      <div v-for="g in groups" :key="g.key" class="cx-group">
        <div class="cx-group-head">
          <span class="cx-group-name">{{ g.label }}</span>
          <span class="cx-group-count">{{ g.items.length }}</span>
        </div>

        <article v-for="(c, i) in g.items" :key="i" class="cx-card">
          <div v-if="!isBlank(c.requiredForm)" class="cx-row">
            <span class="cx-label">接续</span>
            <code class="cx-form">{{ c.requiredForm }}</code>
          </div>

          <div v-if="realsOf(c).length" class="cx-real">
            <span class="cx-label">活用</span>
            <div class="cx-real-wrap">
              <table class="cx-real-table">
                <thead>
                  <tr>
                    <th class="cx-th--stem">原型</th>
                    <th class="cx-th--ko">活用</th>
                    <th v-if="hasRemarkOf(c)" class="cx-th--note">备注</th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="(r, ri) in realsOf(c)" :key="ri">
                    <td class="cx-td--stem">{{ isBlank(r.stem) ? "" : r.stem }}</td>
                    <td class="cx-td--ko">{{ isBlank(r.ko) ? "" : r.ko }}</td>
                    <td v-if="hasRemarkOf(c)" class="cx-td--note">{{ remarkOf(r) }}</td>
                  </tr>
                </tbody>
              </table>
            </div>
          </div>

          <div v-if="!isBlank(c.example)" class="cx-row">
            <span class="cx-label">实例</span>
            <span class="cx-example">{{ c.example }}</span>
          </div>

          <div v-if="!isBlank(c.meaning)" class="cx-row">
            <span class="cx-label">语义</span>
            <span class="cx-text">{{ c.meaning }}</span>
          </div>

          <div v-if="!isBlank(c.tense)" class="cx-row">
            <span class="cx-label">时态</span>
            <span class="cx-chip">{{ TENSE_LABELS[c.tense] || c.tense }}</span>
          </div>

          <div v-if="constraintLines(c).length" class="cx-row cx-constraint">
            <span class="cx-label">限制</span>
            <div class="cx-text">
              <span v-for="(ln, k) in constraintLines(c)" :key="k" class="cx-constraint-line">{{ ln }}</span>
            </div>
          </div>
        </article>
      </div>
    </div>
  </section>
</template>

<style scoped>
.cxs {
  margin: 22px 0 0;
}
.cxs-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-muted);
  margin: 0 0 12px;
}
.cxs-groups {
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.cx-group-head {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 0 0 8px;
}
.cx-group-name {
  font-size: 13px;
  font-weight: 700;
  color: var(--text);
}
.cx-group-count {
  font-size: 10.5px;
  line-height: 16px;
  min-width: 16px;
  padding: 0 5px;
  text-align: center;
  border-radius: 999px;
  background: var(--chip-bg);
  color: var(--text-faint);
}
.cx-card {
  padding: 10px 12px;
  border: 1px solid var(--border-soft);
  border-radius: var(--radius);
  background: var(--panel-inset);
}
.cx-card + .cx-card {
  margin-top: 8px;
}
.cx-row {
  display: flex;
  align-items: baseline;
  gap: 10px;
  padding: 2px 0;
  font-size: 12.5px;
  line-height: 1.65;
}
.cx-label {
  flex: none;
  min-width: 30px;
  color: var(--text-faint);
  font-size: 11px;
}
.cx-form {
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  font-size: 12px;
  color: var(--text);
  background: var(--chip-bg);
  border-radius: var(--radius-sm);
  padding: 1px 6px;
  word-break: break-word;
}
.cx-example {
  color: var(--text);
  font-weight: 600;
  word-break: break-word;
}
.cx-text {
  flex: 1;
  min-width: 0;
  color: var(--text-muted);
  word-break: break-word;
}
.cx-chip {
  color: var(--accent);
  font-size: 11px;
  background: rgba(var(--accent-rgb), 0.1);
  border-radius: 4px;
  padding: 0 6px;
}
.cx-real {
  display: flex;
  align-items: baseline;
  gap: 10px;
  padding: 1px 0;
}
.cx-real-wrap {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 3px;
}
/* 活用表格：一行一个 realization，列 = 原型 / 活用 / （按需）备注 */
.cx-real-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12px;
}
.cx-real-table th {
  font-size: 10.5px;
  font-weight: 600;
  color: var(--text-faint);
  text-align: left;
  padding: 0 10px 3px 0;
  border-bottom: 1px solid var(--border);
}
.cx-real-table td {
  padding: 3px 10px 3px 0;
  border-bottom: 1px solid var(--border-soft);
  word-break: break-word;
  vertical-align: baseline;
}
.cx-real-table tbody tr:last-child td {
  border-bottom: none;
}
.cx-th--stem {
  width: 34%;
}
.cx-th--ko {
  width: 40%;
}
.cx-td--stem {
  color: var(--text-faint);
}
.cx-td--ko {
  color: var(--text);
  font-weight: 700;
  font-size: 12.5px;
}
/* 备注列：不规则/注音等补充信息，用模块强调色弱化呈现（不喧宾夺主） */
.cx-td--note {
  color: var(--sec-accent, var(--accent));
  font-size: 11px;
}
.cx-constraint-line {
  display: block;
}
</style>
