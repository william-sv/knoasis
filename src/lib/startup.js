// Knoasis · 启动路由（记忆用户选择的学科包）
//
// 需求：启动时根据「上次选择」决定停在首页还是直接进入某学科的浏览页：
//  - 从未选择过学科包            → 浏览页，激活最后一个载入的包（不默认载入全部）
//  - 选择过某具体学科包且仍存在   → 浏览页，激活该学科
//  - 选择过「全部」              → 视为「未选定具体学科」，同样激活最后一个载入的包
//                                  （「全部」仅作为运行期切换视图，不作为持久化的启动默认）
//  - 记住的学科包已被删除/停用    → 回落首页，并清掉失效记忆
//
// 本模块为纯逻辑（不依赖 Pinia / Vue / DOM 全局），便于单测：
//  - resolveStartupView：给定「记忆」与「可用学科包」，产出目标视图决策
//  - read/write/clear ActiveSetPref：localStorage 读写（带异常兜底）

/** 记忆当前学科包选择的 localStorage 键 */
export const LS_ACTIVE_SET = "knoasis.activeSet.v1";

/**
 * 读取已记忆的学科包 id。
 * @param {Storage} [storage] 可注入的存储（默认 window.localStorage；不可用时返回 null）
 * @returns {string|null} 'all' | 学科包 id | null（无记忆）
 */
export function readActiveSetPref(storage) {
  const store = storage ?? getDefaultStorage();
  if (!store) return null;
  try {
    const v = store.getItem(LS_ACTIVE_SET);
    return typeof v === "string" && v.length > 0 ? v : null;
  } catch {
    return null;
  }
}

/**
 * 记忆学科包选择（选择「全部」也写入，视为有效选择）。
 * @param {string} setId 'all' | 学科包 id
 * @param {Storage} [storage]
 * @returns {boolean} 是否写入成功
 */
export function writeActiveSetPref(setId, storage) {
  const store = storage ?? getDefaultStorage();
  if (!store || !setId) return false;
  try {
    store.setItem(LS_ACTIVE_SET, String(setId));
    return true;
  } catch {
    return false;
  }
}

/**
 * 清除失效记忆（记住的学科包已被删除/停用）。
 * @param {Storage} [storage]
 * @returns {boolean} 是否清除成功
 */
export function clearActiveSetPref(storage) {
  const store = storage ?? getDefaultStorage();
  if (!store) return false;
  try {
    store.removeItem(LS_ACTIVE_SET);
    return true;
  } catch {
    return false;
  }
}

function getDefaultStorage() {
  if (typeof window === "undefined") return null;
  try {
    return window.localStorage;
  } catch {
    return null;
  }
}

/**
 * 计算启动应停留的视图与激活学科。
 *
 * 注意：调用方必须保证 `sets` 为「已加载完成」的可用学科包列表
 * （scan 已剔除停用包，因此「在 sets 中」即等价于「存在且启用」）。
 *
 * @param {string|null} rememberedId 记忆的选择（'all' | 学科包 id | null）
 * @param {Array<{id: string}>} sets 当前可用（启用）的学科包列表
 * @returns {{ view: 'home'|'browse', activeSetId: 'all'|string, clearMemory: boolean }}
 */
export function resolveStartupView(rememberedId, sets) {
  const list = Array.isArray(sets) ? sets : [];
  const hasSets = list.length > 0;

  // 记住的是「具体学科包」且仍存在且启用 → 直接进入该学科浏览页
  if (rememberedId && rememberedId !== "all") {
    if (list.some((s) => s && s.id === rememberedId)) {
      return { view: "browse", activeSetId: rememberedId, clearMemory: false };
    }
    // 记住的学科包已被删除 / 停用 → 回落首页并清掉失效记忆
    return { view: "home", activeSetId: "all", clearMemory: true };
  }

  // 无记忆 或 记忆「全部」：都视为「未选定具体学科」。
  // 有可用学科包则进入「最后一个载入的包」的浏览页（不再默认载入全部包数据）；
  // 一个包都没有才回落首页（首页承载导入引导）。
  if (hasSets) {
    return { view: "browse", activeSetId: list[list.length - 1].id, clearMemory: false };
  }
  return { view: "home", activeSetId: "all", clearMemory: false };
}
