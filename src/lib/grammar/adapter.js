// Knoasis · 条目模型适配层（学科包通用，docs/Knoasis-学科数据组织与第三方接入方案.md §3.6）
//
// 职责：把 Rust IPC DTO 转成前端 UI 形状，对现有组件/搜索器隐藏 IPC 形状。
//  - EntryItem（列表）→ 条目（EntryList card / 搜索器 / 收藏快照），带 discipline
//  - SubjectMeta → UI set 形状（id/discipline/color/name/levels/types/template）
//  - EntryPayload → 模板渲染数据对象（content JSON 已 view-ready，仅做透传 + images 规范化）
//
// v1 起不再做逐字段 detail 映射 / kr-en 分支：学科包 content 即渲染器消费的键形
// （fields/lists/paragraphs/related），模板 sections 引用的 key 与 content 键直接对齐。

/** level code → rank（未分级 rank 0；I/II/III 兼容历史，其它 code 由 set.levels 提供 label） */
export function rankOf(code) {
  if (code === "I") return 1;
  if (code === "II") return 2;
  if (code === "III") return 3;
  return 0;
}

/** level code → 展示 label（未注册学科回退语义；已注册学科一律用 set.levels 下发的 label） */
export function levelLabelOf(code) {
  if (code === "I") return "TOPIK I";
  if (code === "II") return "TOPIK II";
  return "未分级";
}

/**
 * EntryItem → UI 条目（现有组件/搜索器消费的形状）
 * @param {object} item Rust EntryItem DTO（uid/headword/category/level_code/level_label/tags/summary）
 * @param {string} [discipline] 学科 UI id（由调用方按条目所属学科包显式传入）
 *
 * 缺省为空字符串：学科归属应交由调用方（如 knowledgeSets.fetchRealData 传 s.discipline）提供；
 * 空值表示「未归属具体学科」，UI 层据此不渲染学科 chip（见 EntryDetail）。
 */
export function toEntryItem(item, discipline = "") {
  const code = item.level_code || "";
  return {
    uid: item.uid,
    name: item.headword, // EntryList 标题 / 搜索 name
    type: "grammar", // type 标签（语法集仅此一种；后续多类型包在 set.types 下发）
    category: item.category || "未分类", // EntryList 分组键
    discipline, // 与 set.id 相等（switchSet/filter 依赖）
    level: { code, rank: rankOf(code) },
    tags: Array.isArray(item.tags) ? item.tags : [],
    summary: item.summary || "",
    aliases: [], // 变体不进列表别名（详情展开）
    related: [], // 详情返回后填充（列表级不使用）
    content: null,
    kind: "grammar",
  };
}

/**
 * SubjectMeta → UI set 形状（id/discipline/color/name/levels/types/description/template）
 * SubjectMeta 由 knowledge_list_sets 返回（meta.json + 包内/registry template + DB counts）。
 */
export function toSetShape(meta) {
  const id = meta.discipline || meta.id;
  return {
    id,
    name: meta.name,
    color: meta.color || "#D97706",
    discipline: id,
    levelSystem: meta.level_system || "topik",
    levelSystemLabel: meta.level_system_label || "TOPIK",
    levels: (meta.levels || []).map((l) => ({
      code: l.code,
      label: l.label,
      rank: l.rank,
    })),
    types: (meta.types || []).map((t) => ({ value: t.value, label: t.label })),
    template: meta.template && typeof meta.template === "object" ? meta.template : null,
    description: meta.description || "",
    counts: meta.counts || null,
  };
}

/**
 * EntryPayload → 模板渲染数据对象（content JSON 透传 + images 规范化）。
 * @param {object} payload Rust knowledge_get_entry 返回：
 *   - content：库内 view-ready JSON 对象（fields/lists/paragraphs/related/…，键形与模板 sections 对齐）
 *   - images：已按 image_hidden 过滤 + 解析为本地绝对路径的 [{rel,path}]
 * 返回值直接供 DetailSections/GrammarDetail 消费；content 里的 images（若有 rel 列表）
 * 被 payload.images（绝对路径）覆盖，保证渲染层始终拿到可渲染的 {rel,path}。
 */
export function toDetailView(payload) {
  const content =
    payload && typeof payload.content === "object" && payload.content !== null
      ? payload.content
      : {};
  const images = Array.isArray(payload && payload.images)
    ? payload.images.map((i) => ({
        rel: String((i && i.rel) || ""),
        path: String((i && i.path) || ""),
      }))
    : [];
  return {
    ...content,
    images,
  };
}
