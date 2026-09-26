#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Knoasis · v11 韩语语法 → ko-grammar.knowledgeset 学科包导入器

读取 v11 交付文件（schema 1.3，ko_grammar_v11.json），产出符合 Knoasis knowledge
v1 契约的学科包：ko-grammar.knowledgeset/{meta.json, template.json, knowledge.db}，
供用户在 app 内「导入知识包」安装到用户根（$APPDATA/com.william.knoasis/knowledge）。

> 本发行版**不内置**学科包（commands.rs: builtin_root 注释「当前发行版不内置任何学科包」），
> 故默认输出到仓库 `out/`（可分发产物目录），而非 src-tauri/resources/knowledge。

关键映射（对齐 v11 规范 与 前端模板 sections）：
  - uid            = "ko-grammar:" + sha1(headword)[:12]（与 Rust uid::derive 一致）
  - entries 列     : uid/headword/category/level(→ code I/II)/tags/summary
  - entry_detail.content（view-ready，toDetailView 透传）：
      id, type, pos, speechLevel, aliases, sources, related   ← 条目级字段并入（Rust get_entry 不返回）
      paragraphs.explanation                                  ← content.paragraphs.explanation
      connections[]                                           ← content.connections（按 attachesTo 分组由前端做）
      lists.examples[]  /  lists.senses[]                      ← content.lists.examples / content.senses
      similar[]                                              ← content.similar
      common_errors[]                                        ← content.common_errors
      images: []
  - related 字符串数组：内部 "ko[a-z0-9]{12}" → 映射为对应条目 app uid（可点击跳转）；
    外部 "ext:概念" 原样保留；悬空内部引用保留原串并记 warning。
  - 模板：从 template_registry.json 的 ko_grammar 段原样迁出为包内 template.json
    （registry 为权威回退；包内优先，二者保持一致）。

用法：
  python3 scripts/legacy/import-v11-to-knowledgeset.py \
      --src /Users/william/Downloads/yufa/ko_grammar_v11.json
  python3 scripts/legacy/import-v11-to-knowledgeset.py --src <v11.json> --out /tmp/k
  python3 scripts/legacy/import-v11-to-knowledgeset.py --src <v11.json> --force
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import sqlite3
import sys
from pathlib import Path

SET_ID = "ko-grammar"
PACKAGE_NAME = f"{SET_ID}.knowledgeset"
SCHEMA_VERSION = 1
COLOR = "#D97706"
NAME = "韩语语法"
LANGUAGE = "ko"
KIND = "grammar"
DESCRIPTION = "韩语语法 · TOPIK I/II 分级 · 526 条真实语法点（v11 schema 1.3）"
LEVELS = {
    "system": "topik",
    "label": "TOPIK",
    "values": [
        {"code": "I", "label": "TOPIK I", "rank": 1},
        {"code": "II", "label": "TOPIK II", "rank": 2},
    ],
}
TYPES = [{"value": "grammar", "label": "语法"}]

# v11 level 全称 → 短 code（与 meta.levels.code 对齐；Rust level_label 由 code 反查）
LEVEL_TO_CODE = {"TOPIK I": "I", "TOPIK II": "II", "": ""}

# 旧版 irregularity 缩写码（源数据转换脚本须已改写为韩文标签；包内不得残留）
IRR_CODE_KR = {"ha-irr", "contraction", "r-irr", "d-irr", "s-irr", "eu-drop", "b-irr", "h-irr"}

REPO_ROOT = Path(__file__).resolve().parent.parent.parent  # scripts/legacy/ → repo root
DEFAULT_SRC = REPO_ROOT / "data" / "grammar" / "ko_grammar_v11.json"
# 可分发产物目录：产出的 .knowledgeset 由用户在 app 内「导入知识包」安装到用户根（APPDATA）。
# 注意：本发行版**不内置**任何学科包（见 src-tauri/src/knowledge/commands.rs builtin_root 注释），
# 故不写入 src-tauri/resources/knowledge（该目录随包时不存在）。
DEFAULT_OUT = REPO_ROOT / "out"
REGISTRY = REPO_ROOT / "src-tauri" / "resources" / "grammar" / "template_registry.json"

DDL = """
PRAGMA user_version = 1;
PRAGMA journal_mode = DELETE;

CREATE TABLE entries (
  id         INTEGER PRIMARY KEY,
  uid        TEXT NOT NULL UNIQUE,
  headword   TEXT NOT NULL UNIQUE,
  category   TEXT,
  level      TEXT,
  tags       TEXT,
  summary    TEXT,
  sort       TEXT,
  extra      TEXT
);
CREATE INDEX ix_entries_cat ON entries(category);
CREATE INDEX ix_entries_lv  ON entries(level);

CREATE TABLE entry_detail (
  entry_id INTEGER PRIMARY KEY REFERENCES entries(id) ON DELETE CASCADE,
  content  TEXT NOT NULL
);
"""

# content 顶层允许键（自检用；前端按模板 sections 消费）
ALLOWED_CONTENT_KEYS = {
    "id", "type", "speechLevel", "aliases", "sources", "related",
    "paragraphs", "connections", "lists", "similar", "common_errors", "images",
}


def eprint(*a):
    print(*a, file=sys.stderr)


def derive_uid(headword: str) -> str:
    h = hashlib.sha1(headword.encode("utf-8")).hexdigest()[:12]
    return f"{SET_ID}:{h}"


def s_or_empty(v) -> str:
    return "" if v is None else str(v)


def load_v11(src: Path) -> dict:
    if not src.is_file():
        raise SystemExit(f"源文件不存在: {src}")
    data = json.loads(src.read_text(encoding="utf-8"))
    items = data.get("items")
    if not isinstance(items, list):
        raise SystemExit("v11 文件缺少 items 数组")
    return data


def map_related(related_raw, id_to_uid: dict) -> list:
    """v11 related 字符串数组 → app uid（内部）/ ext:（外部），悬空保留原串。"""
    out = []
    for r in related_raw or []:
        r = s_or_empty(r)
        if not r:
            continue
        if r.startswith("ext:"):
            out.append(r)
            continue
        if re.match(r"^ko[a-z0-9]{12}$", r):
            uid = id_to_uid.get(r)
            out.append(uid if uid else r)  # 悬空则保留原串（自检会记 warning）
            continue
        # 其它形式（理论不存在）原样保留
        out.append(r)
    return out


def build_content(item: dict, id_to_uid: dict) -> dict:
    content = item.get("content") or {}
    raw_related = item.get("related") or []
    return {
        # 条目级字段并入（Rust get_entry 不返回这些）
        "id": s_or_empty(item.get("id")),
        "type": s_or_empty(item.get("type")),
        "speechLevel": s_or_empty(item.get("speechLevel")),
        "aliases": item.get("aliases") or [],
        "sources": item.get("sources") or [],
        "related": map_related(raw_related, id_to_uid),
        # 教学主体
        "paragraphs": content.get("paragraphs") or {},
        "connections": content.get("connections") or [],
        "lists": {
            "examples": (content.get("lists") or {}).get("examples") or [],
            "senses": content.get("senses") or [],
        },
        "similar": content.get("similar") or [],
        "common_errors": content.get("common_errors") or [],
        "images": [],
    }


def build_meta_json(entry_count: int, source_meta: dict | None = None) -> dict:
    sm = source_meta or {}
    return {
        "schema_version": SCHEMA_VERSION,
        "id": SET_ID,
        "name": NAME,
        "version": "11.0.0",
        "language": LANGUAGE,
        "kind": KIND,
        # 优先用源 _meta.description（与 update_url/discipline 等字段同源），缺失回退常量
        "description": (source_meta or {}).get("description") or DESCRIPTION,
        "color": COLOR,
        "levels": LEVELS,
        "types": TYPES,
        "entry_count": entry_count,
        "update_url": sm.get("update_url") or None,
        # ---- 来自源 _meta（v11 规范 14 字段），仅作包内溯源/描述；Rust 解析容忍额外字段 ----
        "template_type": sm.get("template_type", "ko_grammar"),
        "template_version": sm.get("template_version", "1.0"),
        "discipline": sm.get("discipline", ["인문학", "한국어", "문법"]),
        "compiler": sm.get("compiler", ""),
        "original_author": sm.get("original_author", ""),
        "data_source": sm.get("data_source", ""),
        "generated_at": sm.get("generated_at", ""),
    }


def load_template_json() -> dict:
    if not REGISTRY.is_file():
        raise SystemExit(f"缺少 template_registry.json: {REGISTRY}")
    registry = json.loads(REGISTRY.read_text(encoding="utf-8"))
    seg = registry.get("ko_grammar")
    if not isinstance(seg, dict):
        raise SystemExit("template_registry.json 缺少 kr_grammar 段")
    return seg


def make_knowledge_db(target_dir: Path, items: list, id_to_uid: dict) -> dict:
    # 预构建 headword→uid（含冲突确定性后缀）。
    # 注意：冲突解析必须按 items 顺序记录 (final_headword, uid)，绝不能按原始
    # headword 作 key——重复 headword 会让后写入者覆盖前者的映射，导致两条都解析
    # 到同一个 suffixed headword 而触发 UNIQUE 约束冲突。
    entries_meta = []  # 与 items 顺序对齐的 (final_headword, uid)
    used_uids, used_hw = set(), set()
    conflicts = 0
    for it in items:
        hw = it["headword"]
        fhw = hw
        if fhw in used_hw:
            n = 2
            while f"{fhw}-{n}" in used_hw:
                n += 1
            fhw = f"{hw}-{n}"
            conflicts += 1
        uid = derive_uid(fhw)
        while uid in used_uids:
            n += 1
            fhw = f"{hw}-{n}"
            uid = derive_uid(fhw)
            conflicts += 1
        used_hw.add(fhw)
        used_uids.add(uid)
        entries_meta.append((fhw, uid))

    db_path = target_dir / "knowledge.db"
    dst = sqlite3.connect(str(db_path))
    dst.executescript(DDL)

    entry_count = 0
    for idx, it in enumerate(items):
        fhw, uid = entries_meta[idx]
        category = s_or_empty(it.get("category"))
        level = LEVEL_TO_CODE.get(s_or_empty(it.get("level")), s_or_empty(it.get("level")))
        tags = ",".join(it.get("tags") or [])
        summary = s_or_empty(it.get("summary"))
        sort = s_or_empty(it.get("sort"))
        content = build_content(it, id_to_uid)

        dst.execute(
            "INSERT INTO entries(id, uid, headword, category, level, tags, summary, sort) "
            "VALUES (?,?,?,?,?,?,?,?)",
            (entry_count + 1, uid, fhw, category, level, tags, summary, sort),
        )
        dst.execute(
            "INSERT INTO entry_detail(entry_id, content) VALUES (?,?)",
            (entry_count + 1, json.dumps(content, ensure_ascii=False, separators=(",", ":"))),
        )
        entry_count += 1

    dst.commit()
    dst.close()
    return {"entry_count": entry_count, "conflict_count": conflicts}


# ---------------------------------------------------------------------------
# 自检
# ---------------------------------------------------------------------------

def run_selfcheck(pkg_dir: Path, source_items=None) -> dict:
    errors, warnings = [], []
    meta_path = pkg_dir / "meta.json"
    db_path = pkg_dir / "knowledge.db"

    meta = {}
    try:
        meta = json.loads(meta_path.read_text(encoding="utf-8"))
        if meta.get("id") != SET_ID:
            errors.append(f"meta.id={meta.get('id')} != {SET_ID}")
    except Exception as e:  # noqa: BLE001
        errors.append(f"meta.json 不可读: {e}")

    if not db_path.is_file():
        errors.append("缺少 knowledge.db")
        return _report(False, errors, warnings, meta, 0, 0)

    conn = sqlite3.connect(str(db_path))
    try:
        ver = conn.execute("PRAGMA user_version").fetchone()[0]
        if ver != SCHEMA_VERSION:
            errors.append(f"PRAGMA user_version={ver} != {SCHEMA_VERSION}")
        tables = {r[0] for r in conn.execute("SELECT name FROM sqlite_master WHERE type='table'")}
        for t in ("entries", "entry_detail"):
            if t not in tables:
                errors.append(f"缺表 {t}")
        fk = conn.execute("PRAGMA foreign_key_check").fetchall()
        if fk:
            errors.append(f"foreign_key_check 悬空 {len(fk)} 条")

        entry_count = conn.execute("SELECT COUNT(*) FROM entries").fetchone()[0]
        detail_count = conn.execute("SELECT COUNT(*) FROM entry_detail").fetchone()[0]
        if entry_count != detail_count:
            errors.append(f"entries({entry_count}) != entry_detail({detail_count})")

        rows = conn.execute("SELECT uid, headword FROM entries").fetchall()
        seen_uid, seen_hw = set(), set()
        for uid, headword in rows:
            if derive_uid(headword) != uid:
                errors.append(f"uid 重算不一致: {headword} {uid}")
            if uid in seen_uid:
                errors.append(f"uid 重复: {uid}")
            if headword in seen_hw:
                warnings.append(f"headword 重复: {headword}")
            seen_uid.add(uid)
            seen_hw.add(headword)

        declared_codes = {
            v["code"] for v in meta.get("levels", {}).get("values", [])
        } if meta else set()
        c_conn = c_real = c_ex = c_sim = c_err = c_sense = 0
        for (content_raw,) in conn.execute("SELECT content FROM entry_detail").fetchall():
            try:
                c = json.loads(content_raw)
            except Exception as e:  # noqa: BLE001
                errors.append(f"content 非法 JSON: {e}")
                continue
            if not isinstance(c, dict):
                errors.append("content 顶层非对象")
                continue
            extra = set(c.keys()) - ALLOWED_CONTENT_KEYS
            if extra:
                errors.append(f"content 含未允许顶层键: {sorted(extra)}")
            # related 内部引用须均可解析（无悬空 ko…）
            for r in (c.get("related") or []):
                if re.match(r"^ko[a-z0-9]{12}$", r):
                    warnings.append(f"related 悬空内部引用（未映射）: {r}")
            c_conn += len(c.get("connections") or [])
            for conn_item in (c.get("connections") or []):
                c_real += len(conn_item.get("realizations") or [])
            c_ex += len((c.get("lists") or {}).get("examples") or [])
            c_sense += len((c.get("lists") or {}).get("senses") or [])
            c_sim += len(c.get("similar") or [])
            c_err += len(c.get("common_errors") or [])
            # irregularity 自洽校验（§13）：标了 irregularity 必须偏离 stem 直拼；且须为韩文标签
            for conn_item in (c.get("connections") or []):
                for rz in conn_item.get("realizations") or []:
                    irr = rz.get("irregularity")
                    if irr is not None:
                        if irr in IRR_CODE_KR:
                            errors.append(f"irregularity 仍为缩写码（须韩文标签）: {irr}")
                        if rz.get("ko") == rz.get("stem"):
                            errors.append(
                                f"irregularity 标注但 ko==stem 未偏离直拼: "
                                f"headword={headword} stem={rz.get('stem')}"
                            )
        # level code 受控
        if meta:
            for (lv,) in conn.execute(
                "SELECT DISTINCT level FROM entries WHERE level IS NOT NULL AND level<>''"
            ).fetchall():
                if lv not in declared_codes:
                    warnings.append(f"level {lv} 不在 meta.levels 声明内")
        # 与源计数对照
        if source_items is not None:
            s_conn = s_real = s_ex = s_sim = s_err = s_sense = 0
            for it in source_items:
                ct = it.get("content") or {}
                s_conn += len(ct.get("connections") or [])
                for ci in (ct.get("connections") or []):
                    s_real += len(ci.get("realizations") or [])
                s_ex += len((ct.get("lists") or {}).get("examples") or [])
                s_sense += len(ct.get("senses") or [])
                s_sim += len(ct.get("similar") or [])
                s_err += len(ct.get("common_errors") or [])
            mism = []
            if entry_count != len(source_items):
                mism.append(f"entries {entry_count} != 源 {len(source_items)}")
            if c_conn != s_conn:
                mism.append(f"connections {c_conn} != 源 {s_conn}")
            if c_real != s_real:
                mism.append(f"realizations {c_real} != 源 {s_real}")
            if c_ex != s_ex:
                mism.append(f"examples {c_ex} != 源 {s_ex}")
            if c_sim != s_sim:
                mism.append(f"similar {c_sim} != 源 {s_sim}")
            if c_err != s_err:
                mism.append(f"common_errors {c_err} != 源 {s_err}")
            if c_sense != s_sense:
                mism.append(f"senses {c_sense} != 源 {s_sense}")
            for m in mism:
                errors.append(f"计数不一致: {m}")
    finally:
        conn.close()

    ok = not errors
    return _report(ok, errors, warnings, meta, entry_count, 0)


def _report(ok, errors, warnings, meta, entry_count, conflict_count):
    return {
        "ok": ok,
        "errors": errors,
        "warnings": warnings,
        "meta": {
            "id": meta.get("id") if meta else None,
            "version": meta.get("version") if meta else None,
            "schema_version": meta.get("schema_version") if meta else None,
            "entry_count": meta.get("entry_count") if meta else None,
        },
        "entry_count": entry_count,
        "conflict_count": conflict_count,
    }


def main() -> int:
    ap = argparse.ArgumentParser(description="v11 韩语语法 → kr-grammar.knowledgeset")
    ap.add_argument("--src", type=Path, default=DEFAULT_SRC, help="v11 json 路径")
    ap.add_argument("--out", type=Path, default=DEFAULT_OUT, help="输出根目录")
    ap.add_argument("--force", action="store_true", help="覆盖重建")
    ap.add_argument("--no-sync", action="store_true", help="不复制到 APPDATA 用户根")
    args = ap.parse_args()

    src = Path(args.src)
    out_root = Path(args.out)
    data = load_v11(src)
    items = data["items"]

    # 预构建 v11 id → app uid（供 related 映射）
    id_to_uid = {}
    for it in items:
        vid = s_or_empty(it.get("id"))
        id_to_uid[vid] = derive_uid(it["headword"])

    pkg_dir = out_root / PACKAGE_NAME
    if pkg_dir.exists():
        if not args.force:
            eprint("目标已存在；使用 --force 覆盖重建")
            return 2
        shutil.rmtree(pkg_dir)
    pkg_dir.mkdir(parents=True, exist_ok=True)

    stats = make_knowledge_db(pkg_dir, items, id_to_uid)

    (pkg_dir / "meta.json").write_text(
        json.dumps(build_meta_json(stats["entry_count"], data.get("_meta")), ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    (pkg_dir / "template.json").write_text(
        json.dumps(load_template_json(), ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )

    report = run_selfcheck(pkg_dir, source_items=items)
    report["conflict_count"] = stats["conflict_count"]
    print(json.dumps(report, ensure_ascii=False, indent=2))

    if not report["ok"]:
        eprint("导入自检未通过，检查 errors。")
        return 3

    print(f"[done] 包已产出: {pkg_dir}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
