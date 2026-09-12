// QA 收尾轮补充：⌘K 快捷键纯逻辑边界（不改实现者 shortcuts.test.js）
// 运行：node --test src/lib/__tests__/qa-shortcuts-edge.test.js
import { test } from "node:test";
import assert from "node:assert/strict";
import { isEditableTarget, isSearchShortcut, shouldFocusSearch } from "../shortcuts.js";

function el(tagName, { contentEditable = false } = {}) {
  return { tagName, isContentEditable: contentEditable };
}
function ev(over = {}) {
  return { key: "k", metaKey: false, ctrlKey: false, ...over };
}

test("QA：无修饰键的 k 不应触发（metaKey=false 且 ctrlKey=false）", () => {
  assert.equal(isSearchShortcut(ev()), false);
  assert.equal(isSearchShortcut(ev({ metaKey: undefined, ctrlKey: undefined })), false);
});

test("QA：ctrlKey 路径可用（Windows/Linux）", () => {
  assert.equal(isSearchShortcut(ev({ ctrlKey: true })), true);
  assert.equal(shouldFocusSearch(ev({ ctrlKey: true }), el("BODY")), true);
});

test("QA：meta+ctrl 同时按下（跨平台宽松）仍应命中", () => {
  assert.equal(isSearchShortcut(ev({ metaKey: true, ctrlKey: true })), true);
});

test("QA：大小写 K 均命中，非 k 键不命中", () => {
  assert.equal(isSearchShortcut(ev({ metaKey: true, key: "K" })), true);
  assert.equal(isSearchShortcut(ev({ metaKey: true, key: "м" })), false);
  assert.equal(isSearchShortcut(ev({ metaKey: true, key: "Enter" })), false);
});

test("QA：带 shift/alt 的 Cmd+Shift+K 也会命中（仅校验 meta/ctrl + key）", () => {
  assert.equal(isSearchShortcut(ev({ metaKey: true, shiftKey: true })), true);
  assert.equal(isSearchShortcut(ev({ metaKey: true, altKey: true })), true);
});

test("QA：焦点就在搜索框自身(input) 时按 ⌘K 不抢占", () => {
  assert.equal(shouldFocusSearch(ev({ metaKey: true }), el("INPUT")), false);
});

test("QA：isEditableTarget 大小写不敏感 + 显式 contentEditable=false", () => {
  assert.equal(isEditableTarget(el("InPut")), true);
  assert.equal(isEditableTarget(el("TEXTAREA")), true);
  assert.equal(isEditableTarget(el("DIV", { contentEditable: false })), false);
  assert.equal(isEditableTarget({ tagName: 123 }), false);
  assert.equal(isEditableTarget("BODY"), false);
});

test("QA：shouldFocusSearch 传入 null/undefined 事件均安全", () => {
  assert.equal(shouldFocusSearch(null, el("BODY")), false);
  assert.equal(shouldFocusSearch(undefined, el("BODY")), false);
  assert.equal(shouldFocusSearch(ev({ metaKey: true }), undefined), true); // 无焦点元素 → 视为非输入态
});
