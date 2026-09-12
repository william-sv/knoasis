// 沙箱验证：userdata 迁移载荷纯函数（对齐设计 §4.5.3 / §7.1）
// 运行：node --test src/lib/__tests__/userdata.test.js
import { test } from "node:test";
import assert from "node:assert/strict";
import { buildMigratePayload } from "../userdata-payload.js";

test("buildMigratePayload 把 UI 快照收藏映射为 snake_case FavoriteInput", () => {
  const legacyFavs = [
    {
      uid: "legacy:item-1",
      name: "示例条目",
      type: "grammar",
      discipline: "kr-grammar",
      category: "助词",
      levelCode: "II",
      summary: "旧数据快照示例",
      favoritedAt: 1700000000000,
    },
  ];
  const payload = buildMigratePayload(legacyFavs, {});
  assert.equal(payload.favorites.length, 1);
  assert.equal(payload.favorites[0].uid, "legacy:item-1");
  assert.equal(payload.favorites[0].level_code, "II");
  assert.equal(payload.favorites[0].category, "助词");
  assert.ok(!("favoritedAt" in payload.favorites[0]));
  assert.equal(payload.notes.length, 0);
});

test("buildMigratePayload 把笔记 Record 映射为 NoteInput（含 updated_at 归一）", () => {
  const legacyNotes = {
    "legacy:item-1": { content: "例句速记：…", updatedAt: 1700000000000 },
    "legacy:item-2": { content: "", updatedAt: 1700000000001 },
  };
  const payload = buildMigratePayload([], legacyNotes);
  assert.equal(payload.notes.length, 2);
  const first = payload.notes.find((n) => n.uid === "legacy:item-1");
  assert.equal(first.content, "例句速记：…");
  assert.equal(first.updated_at, 1700000000000);
});

test("buildMigratePayload 非法输入不抛错（空/非数组）", () => {
  assert.deepEqual(buildMigratePayload(null, null), { favorites: [], notes: [] });
  assert.deepEqual(buildMigratePayload(undefined, "garbage"), { favorites: [], notes: [] });
});
