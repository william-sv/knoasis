// 单测：顶栏「聚焦搜索」快捷键纯逻辑（Cmd/Ctrl + K）
// 运行：node --test src/lib/__tests__/shortcuts.test.js
import { test } from "node:test";
import assert from "node:assert/strict";

import { isEditableTarget, isSearchShortcut, shouldFocusSearch } from "../shortcuts.js";

/** 构造最小伪造元素（只含判定所需字段） */
function fakeEl(tagName, { contentEditable = false } = {}) {
  return { tagName, isContentEditable: contentEditable };
}

/** 构造最小伪造按键事件 */
function keyEvent(over = {}) {
  return { key: "k", metaKey: false, ctrlKey: false, ...over };
}

test("isEditableTarget 识别输入类元素与 contenteditable，其余为非编辑态", () => {
  assert.equal(isEditableTarget(fakeEl("INPUT")), true);
  assert.equal(isEditableTarget(fakeEl("textarea")), true);
  assert.equal(isEditableTarget(fakeEl("SELECT")), true);
  assert.equal(isEditableTarget(fakeEl("DIV", { contentEditable: true })), true);
  assert.equal(isEditableTarget(fakeEl("DIV")), false);
  assert.equal(isEditableTarget(fakeEl("BODY")), false);
  assert.equal(isEditableTarget(null), false);
  assert.equal(isEditableTarget(undefined), false);
  assert.equal(isEditableTarget({}), false);
});

test("isSearchShortcut 仅匹配 Cmd/Ctrl + K（大小写不敏感）", () => {
  assert.equal(isSearchShortcut(keyEvent({ metaKey: true })), true);
  assert.equal(isSearchShortcut(keyEvent({ ctrlKey: true })), true);
  assert.equal(isSearchShortcut(keyEvent({ metaKey: true, key: "K" })), true);
  assert.equal(isSearchShortcut(keyEvent({ ctrlKey: true, key: "K" })), true);
  // 无修饰键 / 其它键 / Cmd+J → 不匹配
  assert.equal(isSearchShortcut(keyEvent()), false);
  assert.equal(isSearchShortcut(keyEvent({ metaKey: true, key: "j" })), false);
  assert.equal(isSearchShortcut(keyEvent({ ctrlKey: true, key: " " })), false);
  assert.equal(isSearchShortcut(null), false);
});

test("shouldFocusSearch：非输入态命中→处理；输入态与未命中→不处理", () => {
  const hit = keyEvent({ metaKey: true });
  // 非输入态：处理（含无焦点 activeElement 为 body / null）
  assert.equal(shouldFocusSearch(hit, fakeEl("BODY")), true);
  assert.equal(shouldFocusSearch(hit, null), true);
  assert.equal(shouldFocusSearch(hit, fakeEl("DIV")), true);
  // 输入态：不抢占
  assert.equal(shouldFocusSearch(hit, fakeEl("INPUT")), false);
  assert.equal(shouldFocusSearch(hit, fakeEl("TEXTAREA")), false);
  assert.equal(shouldFocusSearch(hit, fakeEl("DIV", { contentEditable: true })), false);
  // 未命中组合键：永不处理
  assert.equal(shouldFocusSearch(keyEvent(), fakeEl("BODY")), false);
  assert.equal(shouldFocusSearch(keyEvent({ metaKey: true, key: "j" }), fakeEl("BODY")), false);
});
