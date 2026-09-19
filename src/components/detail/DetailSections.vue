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

defineProps({
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
</script>

<template>
  <div class="detail-sections">
    <template v-if="template && template.sections && template.sections.length">
      <template v-for="section in template.sections" :key="section.key">
        <!-- meta：头部元数据条由 GrammarDetail 挂载，这里跳过 -->
        <template v-if="section.type === 'meta'"></template>

        <!-- fields：字段组 -->
        <DetailFieldTable
          v-else-if="section.type === 'fields'"
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

        <!-- 其它 list：空数组整节隐藏 -->
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
      </template>
    </template>

    <p v-else class="no-template">（此条目暂无结构化详情模板）</p>
  </div>
</template>

<style scoped>
.detail-sections {
  margin-top: 2px;
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
