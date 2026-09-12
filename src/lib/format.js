// Knoasis UI 原型 · 通用格式化 / 工具

import { findSetById, getSets } from "./data-registry.js";

// 类型展示文案：优先取已注册学科 types 段（真实数据自带 label），
// 找不到时回落内置最小词表（仅语法等稳定标签），仍找不到则原样返回 type。
const BUILTIN_TYPE_LABELS = { grammar: "语法" };

export function typeLabel(type) {
  if (type == null || type === "") return "";
  for (const set of getSets()) {
    const t = (set.types || []).find((x) => x.value === type);
    if (t && t.label) return t.label;
  }
  return BUILTIN_TYPE_LABELS[type] ?? type;
}

// 学科展示名，'all' 时为全部（emoji 清理后已无 icon 字段，仅取 name）
export function setDisplayName(setId) {
  if (!setId || setId === "all") return "全部学科";
  const set = findSetById(setId);
  return set ? set.name : setId;
}

// 条目的「学科色」；找不到返回中性色
export function setColorOf(discipline) {
  const set = findSetById(discipline);
  return set ? set.color : "#9aa3ae";
}

// 把 HTML 正文粗略转为纯文本（复制 / 摘要用；非 sanitize 用途）
export function stripHtml(html) {
  return String(html ?? "")
    .replace(/<br\s*\/?>/gi, "\n")
    .replace(/<\/(p|li|h[1-6]|tr|div)>/gi, "\n")
    .replace(/<[^>]+>/g, "")
    .replace(/&nbsp;/g, " ")
    .replace(/&amp;/g, "&")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .join("\n");
}

// 复制到剪贴板（含降级方案），返回是否成功
export async function copyText(text) {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    try {
      const ta = document.createElement("textarea");
      ta.value = text;
      ta.style.position = "fixed";
      ta.style.opacity = "0";
      document.body.appendChild(ta);
      ta.focus();
      ta.select();
      const ok = document.execCommand("copy");
      document.body.removeChild(ta);
      return ok;
    } catch {
      return false;
    }
  }
}

// 触发浏览器下载 JSON（导出收藏/笔记用）
export function downloadJson(filename, payload) {
  const blob = new Blob([JSON.stringify(payload, null, 2)], {
    type: "application/json;charset=utf-8",
  });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

// 简单时间格式化
export function formatTime(ts) {
  if (!ts) return "";
  const d = new Date(ts);
  const p = (n) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(
    d.getHours(),
  )}:${p(d.getMinutes())}`;
}
