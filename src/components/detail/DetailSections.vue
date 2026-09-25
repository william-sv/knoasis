<script setup>
// 详情 · 模板 sections 遍历器（通用渲染，kr_grammar / en_grammar 共用）
// 规则（对齐 docs/Knoasis-英语语法接入设计.md §6）：
//  - fields：空字符串/空数组字段跳过（DetailFieldTable）
//  - list：子表为空数组 → 整节隐藏；key=related 交给 DetailRelatedBlock（或 v8 数组态 chips）
//  - paragraph：整段讲解（view.paragraphs[key]，空白整节隐藏，DetailParagraph）
//  - images：图解网格（view.images，空数组整节隐藏，DetailImageGrid；entryUid 供删图 IPC）
//  - connections / patterns / similar：二维子表（DetailTable，列定义见 COLUMN_SETS）
//  - examples：例句表（DetailExamples，含 audio 播放）
//  - common_errors：易错点（DetailErrors）
import DetailFieldTable from "./DetailFieldTable.vue";
import DetailListBlock from "./DetailListBlock.vue";
import DetailRelatedBlock from "./DetailRelatedBlock.vue";
import DetailParagraph from "./DetailParagraph.vue";
import DetailImageGrid from "./DetailImageGrid.vue";
import DetailMeta from "./DetailMeta.vue";
import DetailTable from "./DetailTable.vue";
import DetailExamples from "./DetailExamples.vue";
import DetailErrors from "./DetailErrors.vue";
import DetailConnections from "./DetailConnections.vue";
import DetailRelatedChips from "./DetailRelatedChips.vue";

// 注意：必须用 const props 接住 —— 下方 rowsFor() 读取 props.view；
// 若只写裸 defineProps(...)，脚本作用域内没有 props 变量，examples/patterns/similar/antonyms
// 这些需要 rowsFor() 的节会在渲染期抛 ReferenceError，导致整个模板详情子树渲染中断（正文空白）。
const props = defineProps({
  template: { type: Object, default: null }, // set.template（kr_grammar / en_grammar 段）
  view: { type: Object, default: null }, // adapter.toDetailView(payload)（content 透传后的模板数据对象）
  entryUid: { type: String, default: "" }, // 当前详情条目 uid（DetailImageGrid 删图入参）
});

// 列表“主行”字段：用于 examples/meanings 等展示主内容不带 label（kr 默认）
const headKeyBySection = {
  examples: "ko",
  meanings: "meaning_zh",
  usages: "title",
  scenes: "scene",
  cautions: "caution",
  collocations: "word",
};

// 二维子表列定义（connections / patterns / similar）
const COLUMN_SETS = {
  connections: [
    { key: "attachesTo", label: "接" },
    { key: "requiredForm", label: "前置形态", mono: true },
    { key: "example", label: "实例" },
    { key: "meaning", label: "语义" },
  ],
  patterns: [
    { key: "pattern", label: "结构公式", mono: true },
    { key: "ko", label: "韩文" },
    { key: "zh", label: "中文" },
  ],
  similar: [
    { key: "headword", label: "近义" },
    { key: "difference", label: "辨析" },
  ],
  // 相反语法：模块已具备，view.antonyms 无值则整节隐藏（v11 当前无该字段）
  antonyms: [
    { key: "headword", label: "相反" },
    { key: "difference", label: "辨析" },
  ],
};

// 取某 section 对应的数据数组（examples/patterns 在 view.lists 下，其余在 view 顶层）
function rowsFor(section) {
  const v = props.view || {};
  if (section.type === "examples" || section.type === "patterns") {
    const list = v.lists || {};
    return Array.isArray(list[section.key]) ? list[section.key] : [];
  }
  return Array.isArray(v[section.key]) ? v[section.key] : [];
}

function paragraphKey(section) {
  return section.fieldKey || section.key;
}
function headKeyFor(section, view) {
  if (section.type !== "list") return "";
  if (headKeyBySection[section.key]) {
    const items = view && view.lists ? view.lists[section.key] : [];
    const first = Array.isArray(items) && items.length ? items[0] : null;
    if (headKeyBySection[section.key] === "ko" && first && "en" in first && !("ko" in first)) {
      return "en";
    }
    return headKeyBySection[section.key];
  }
  return "";
}
// v8 的 related 可能是 headword 数组（直接渲染 chips），旧结构是 {resolved, pending}
function isRelatedArray(view) {
  return view && view.related && Array.isArray(view.related);
}

// 各模块固定配色（淡暖色系，直接写死：c = 主色 hex，rgb = 同色 RGB 通道串）。
// rgb 供 rgba(var(--sec-accent-rgb), a) 兼容写法取浅底/描边，不依赖 color-mix。
const SEC_THEME = {
  explanation: { c: "#E6A85C", rgb: "230, 168, 92" }, // 语法释义 · 暖杏金
  connections_grouped: { c: "#D98A4B", rgb: "217, 138, 75" }, // 活用/接续 · 琥珀
  examples: { c: "#D96A4E", rgb: "217, 106, 78" }, // 例句 · 珊瑚
  common_errors: { c: "#CC5B4E", rgb: "204, 91, 78" }, // 易错点 · 砖红
  similar: { c: "#C9824E", rgb: "201, 130, 78" }, // 近似语法 · 焦糖
  senses: { c: "#B9923C", rgb: "185, 146, 60" }, // 义项 · 橄榄金
  related: { c: "#9C8C7E", rgb: "156, 140, 126" }, // 相关语法 · 暖灰褐
};
const SEC_THEME_FALLBACK = { c: "#C9824E", rgb: "201, 130, 78" };
function secStyle(section) {
  const t = (section && SEC_THEME[section.key]) || SEC_THEME_FALLBACK;
  return { "--sec-accent": t.c, "--sec-accent-rgb": t.rgb };
}
// 该 section 是否真的有内容可展示：为空则整块（含边框）都不渲染，避免空色条。
function sectionHasContent(section) {
  const v = props.view || {};
  const key = section && section.key;
  const ptype = section && section.type;
  if (ptype === "paragraph") {
    const t = v.paragraphs ? v.paragraphs[paragraphKey(section)] : "";
    return !!(t && String(t).trim());
  }
  if (ptype === "images") return Array.isArray(v.images) && v.images.length > 0;
  if (ptype === "connections_grouped")
    return Array.isArray(v.connections) && v.connections.length > 0;
  if (ptype === "related")
    return Array.isArray(v.related) && v.related.length > 0;
  if (ptype === "list" && key === "related") {
    if (isRelatedArray(v))
      return Array.isArray(v.related) && v.related.length > 0;
    const rel = v.related || {};
    return !!(Array.isArray(rel.resolved) && rel.resolved.length) ||
      !!(Array.isArray(rel.pending) && rel.pending.length);
  }
  if (ptype === "fields") {
    const f = v.fields || {};
    return Object.keys(f).some((k) => f[k] != null && String(f[k]).trim() !== "");
  }
  // list / examples / common_errors / connections / patterns / similar / antonyms
  return rowsFor(section).length > 0;
}
</script>

<template>
  <div class="detail-sections">
    <template v-if="template && template.sections && template.sections.length">
      <template v-for="section in template.sections" :key="section.key">
        <!-- meta：头部元数据条由 GrammarDetail 挂载，这里跳过 -->
        <template v-if="section.type === 'meta'"></template>

        <!-- 其它 section：统一包裹为带强调色的模块卡片（彩色边框 + 标题色；无内容整块隐藏） -->
        <section
          v-else-if="sectionHasContent(section)"
          class="ds-block"
          :style="secStyle(section)"
        >
        <!-- fields：字段组 -->
        <DetailFieldTable
          v-if="section.type === 'fields'"
          :label="section.label"
          :fields="section.fields || []"
          :view-fields="view ? view.fields : {}"
        />

        <!-- related：v8 数组态（headword 列表）→ chips；旧 {resolved,pending} → DetailRelatedBlock -->
        <template v-else-if="section.type === 'list' && section.key === 'related'">
          <DetailRelatedBlock
            v-if="!isRelatedArray(view)"
            :label="section.label"
            :related="view && view.related ? view.related : { resolved: [], pending: [] }"
          />
          <section v-else-if="view.related.length" class="rel-section">
            <h2 class="lsection-title">{{ section.label }}</h2>
            <div class="rel-chips">
              <span v-for="r in view.related" :key="r" class="rel-chip-static">{{ r }}</span>
            </div>
          </section>
        </template>

        <!-- paragraph：整段讲解（空白自动隐藏） -->
        <DetailParagraph
          v-else-if="section.type === 'paragraph'"
          :label="section.label"
          :text="view && view.paragraphs ? view.paragraphs[paragraphKey(section)] : ''"
          :format="section.format"
        />

        <!-- connections_grouped：语法用法详解，按 动词/形容词/名词 三块分组（v11） -->
        <DetailConnections
          v-else-if="section.type === 'connections_grouped'"
          :label="section.label"
          :connections="view && Array.isArray(view.connections) ? view.connections : []"
        />

        <!-- related（v11 字符串数组）：内部 uid 可跳转 + 外部 ext: 概念 chip -->
        <DetailRelatedChips
          v-else-if="section.type === 'related'"
          :label="section.label"
          :related="view && Array.isArray(view.related) ? view.related : []"
        />

        <!-- connections / patterns / similar / antonyms：二维子表（空行/空数组整节隐藏） -->
        <DetailTable
          v-else-if="['connections', 'patterns', 'similar', 'antonyms'].includes(section.type)"
          :label="section.label"
          :columns="COLUMN_SETS[section.type]"
          :rows="rowsFor(section)"
        />

        <!-- examples：例句表（含 audio） -->
        <DetailExamples
          v-else-if="section.type === 'examples'"
          :label="section.label"
          :items="rowsFor(section)"
        />

        <!-- common_errors：易错点 -->
        <DetailErrors
          v-else-if="section.type === 'common_errors'"
          :label="section.label"
          :items="rowsFor(section)"
        />

        <!-- images：图解网格（空数组整节隐藏） -->
        <DetailImageGrid
          v-else-if="section.type === 'images' && view && Array.isArray(view.images) && view.images.length"
          :label="section.label || '图解'"
          :images="view.images"
          :entry-uid="entryUid"
        />

        <!-- 其它 list：空数组整节隐藏（senses 等顶层 list 节读 view.lists[key]） -->
        <DetailListBlock
          v-else-if="
            section.type === 'list' &&
            view &&
            view.lists &&
            Array.isArray(view.lists[section.key]) &&
            view.lists[section.key].length
          "
          :label="section.label"
          :items="view.lists[section.key]"
          :item-fields="section.itemFields || []"
          :head-key="headKeyFor(section, view)"
        />
        </section>
      </template>
    </template>

    <p v-else class="no-template">（此条目暂无结构化详情模板）</p>
  </div>
</template>

<style scoped>
.detail-sections {
  margin-top: 2px;
}
/* 模块容器：无外边框、无底色（模块身份由「彩色标题 + 下划线」承担，内部卡片自带描边） */
.ds-block {
  margin: 18px 0 0;
  padding: 0;
}
/* 模块标题：强调色标题 + 同色下划线（明确的模块身份） */
.ds-block :deep(h2) {
  color: var(--sec-accent);
  font-size: 12.5px;
  font-weight: 700;
  letter-spacing: 0.3px;
  margin: 0 0 11px;
  padding: 0 0 7px;
  border-bottom: 1px solid rgba(var(--sec-accent-rgb), 0.4);
}
/* 消除子组件自带的顶部外边距（卡片 padding 已提供间距） */
.ds-block :deep(.psection),
.ds-block :deep(.cxs),
.ds-block :deep(.exsection),
.ds-block :deep(.esection),
.ds-block :deep(.tsection),
.ds-block :deep(.lsection),
.ds-block :deep(.relchips),
.ds-block :deep(.rel-section) {
  margin-top: 0;
}
/* 内部卡片：完整描边的浅底卡片（与模块色块形成分层，不再用左侧色条） */
.ds-block :deep(.paragraph-body),
.ds-block :deep(.ex-item),
.ds-block :deep(.list-item),
.ds-block :deep(.cx-card),
.ds-block :deep(.err-item) {
  border: 1px solid rgba(var(--sec-accent-rgb), 0.35);
  background: var(--panel);
}
/* 例句/相关 等组件的交互色也跟随模块强调色 */
.ds-block :deep(.ex-audio) {
  color: var(--sec-accent);
  border-color: rgba(var(--sec-accent-rgb), 0.4);
}
.ds-block :deep(.ex-audio:hover) {
  background: rgba(var(--sec-accent-rgb), 0.12);
  border-color: rgba(var(--sec-accent-rgb), 0.45);
}
.ds-block :deep(.rc-chip:not(:disabled):hover) {
  border-color: rgba(var(--sec-accent-rgb), 0.4);
}
.ds-block :deep(.rc-chip:not(:disabled):hover .rc-arrow) {
  color: var(--sec-accent);
}
.no-template {
  margin: 18px 0 0;
  font-size: 12px;
  color: var(--text-faint);
}
.lsection-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-muted);
  margin: 0 0 10px;
}
.rel-section {
  margin: 22px 0 0;
}
.rel-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
.rel-chip-static {
  display: inline-flex;
  align-items: center;
  height: 28px;
  padding: 0 10px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: var(--panel);
  font-size: 12px;
  color: var(--text);
}
</style>
