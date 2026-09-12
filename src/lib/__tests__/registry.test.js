// 沙箱验证：运行时注册表（对齐设计 §3.1 / §7.1）
// 运行：node --test src/lib/__tests__/registry.test.js
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  registerSets,
  registerEntries,
  findSetById,
  findEntryByUid,
  levelMetaForEntry,
} from "../data-registry.js";

test("registerSets 替换生效，findSetById 命中新注册学科", () => {
  registerSets([
    {
      id: "kr-grammar",
      name: "韩语语法",
      color: "#D97706",
      discipline: "kr-grammar",
      levels: [
        { code: "I", label: "TOPIK I", rank: 1 },
        { code: "II", label: "TOPIK II", rank: 2 },
      ],
    },
  ]);
  const set = findSetById("kr-grammar");
  assert.ok(set);
  assert.equal(set.name, "韩语语法");
  assert.equal(findSetById("not-a-set"), undefined);
});

test("registerEntries + findEntryByUid", () => {
  registerEntries([
    { uid: "kr-grammar:a1b2c3d4e5f6", name: "N마저", discipline: "kr-grammar", level: { code: "II", rank: 2 } },
  ]);
  assert.equal(findEntryByUid("kr-grammar:a1b2c3d4e5f6").name, "N마저");
  assert.equal(findEntryByUid("nope"), undefined);
});

test("levelMetaForEntry 对未分级 code '' 兜底返回「未分级」", () => {
  const meta = levelMetaForEntry({
    uid: "u1",
    discipline: "kr-grammar",
    level: { code: "", rank: 0 },
  });
  assert.equal(meta.label, "未分级");
  assert.equal(meta.rank, 0);
});

test("levelMetaForEntry 分级条目取 set.levels 的 label", () => {
  const meta = levelMetaForEntry({
    uid: "u2",
    discipline: "kr-grammar",
    level: { code: "I", rank: 1 },
  });
  assert.equal(meta.label, "TOPIK I");
});

test("空注册（清空）后查询不抛错：findSetById/findEntryByUid 返回 undefined", () => {
  registerSets([]);
  registerEntries([]);
  assert.equal(findSetById("kr-grammar"), undefined);
  assert.equal(findEntryByUid("kr-grammar:a1b2c3d4e5f6"), undefined);
  // 未分级条目在学科未注册时仍兜底返回「未分级」，不抛错
  const ungraded = levelMetaForEntry({
    uid: "u3",
    discipline: "kr-grammar",
    level: { code: "", rank: 0 },
  });
  assert.equal(ungraded.label, "未分级");
  assert.equal(ungraded.rank, 0);
});
