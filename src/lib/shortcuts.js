// Knoasis · 键盘快捷键纯逻辑（可单测，无 DOM 依赖）
//
// 目前服务顶栏「聚焦搜索」：Cmd（macOS）/ Ctrl（Windows/Linux）+ K。
// 拆成纯函数便于在 `node --test` 下验证；组件层只负责 DOM 副作用
// （preventDefault / focus / select），便于测试与复用。

/**
 * 判定某元素是否为「文本输入目标」（输入框 / 文本域 / 下拉 / contenteditable）。
 * 命中时不应抢占按键，避免破坏用户正在进行的输入。
 * @param {{tagName?: string, isContentEditable?: boolean}|null|undefined} el
 * @returns {boolean}
 */
export function isEditableTarget(el) {
  if (!el || typeof el !== "object") return false;
  const tag = typeof el.tagName === "string" ? el.tagName.toLowerCase() : "";
  if (tag === "input" || tag === "textarea" || tag === "select") return true;
  if (el.isContentEditable === true) return true;
  return false;
}

/**
 * 是否为「聚焦搜索」组合键：Cmd（macOS）/ Ctrl（其它平台）+ K，K 大小写不敏感。
 * @param {{key?: string, metaKey?: boolean, ctrlKey?: boolean}|null|undefined} e
 * @returns {boolean}
 */
export function isSearchShortcut(e) {
  if (!e || typeof e !== "object") return false;
  const key = typeof e.key === "string" ? e.key.toLowerCase() : "";
  if (key !== "k") return false;
  return e.metaKey === true || e.ctrlKey === true;
}

/**
 * 是否应在本组件处理该按键：命中聚焦搜索快捷键，且当前焦点不在可编辑元素内。
 * 输入态不抢占（让输入框/浏览器保持默认行为）。
 * @param {{key?: string, metaKey?: boolean, ctrlKey?: boolean}|null|undefined} e
 * @param {{tagName?: string, isContentEditable?: boolean}|null|undefined} activeElement document.activeElement
 * @returns {boolean}
 */
export function shouldFocusSearch(e, activeElement) {
  if (!isSearchShortcut(e)) return false;
  if (isEditableTarget(activeElement)) return false;
  return true;
}
