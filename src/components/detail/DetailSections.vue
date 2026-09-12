<script setup>
// 详情 · 模板 sections 遍历器（通用渲染，kr_grammar / en_grammar 共用）
// 规则（对齐 docs/Knoasis-英语语法接入设计.md §6）：
//  - fields：空字符串/空数组字段跳过（DetailFieldTable）
//  - list：子表为空数组 → 整节隐藏；key=related 交给 DetailRelatedBlock
//  - paragraph：整段讲解（view.paragraphs[key]，空白整节隐藏，DetailParagraph）
//  - images：图解网格（view.images，空数组整节隐藏，DetailImageGrid；entryUid 供删图 IPC）
import DetailFieldTable from "./DetailFieldTable.vue";
import DetailListBlock from "./DetailListBlock.vue";
import DetailRelatedBlock from "./DetailRelatedBlock.vue";
import DetailParagraph from "./DetailParagraph.vue";
import DetailImageGrid from "./DetailImageGrid.vue";

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

// examples 行既有 ko（韩语）又有 en（英语）：按行实际字段取主行 key，kr 行为不变。
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

function paragraphKey(section) {
  return section.fieldKey || section.key;
}
</script>

<template>
  <div class="detail-sections">
    <template v-if="template && template.sections && template.sections.length">
      <template v-for="section in template.sections" :key="section.key">
        <!-- fields：字段组 -->
        <DetailFieldTable
          v-if="section.type === 'fields'"
          :label="section.label"
          :fields="section.fields || []"
          :view-fields="view ? view.fields : {}"
        />

        <!-- related：关联语法专用（chips + 未收录折叠） -->
        <DetailRelatedBlock
          v-else-if="section.type === 'list' && section.key === 'related'"
          :label="section.label"
          :related="view && view.related ? view.related : { resolved: [], pending: [] }"
        />

        <!-- paragraph：整段讲解（空白自动隐藏） -->
        <DetailParagraph
          v-else-if="section.type === 'paragraph'"
          :label="section.label"
          :text="view && view.paragraphs ? view.paragraphs[paragraphKey(section)] : ''"
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
</style>
