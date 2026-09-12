#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Knoasis · 一次性迁移工具：旧 grammar.db(kr_grammar) → kr-grammar.knowledgeset 学科包

设计：docs/Knoasis-学科数据组织与第三方接入方案.md §4.2 / §5（契约 v1）。
  - 只读打开 data/grammar/grammar.db（源）；另写目标（默认 src-tauri/resources/knowledge）
  - 目标包：kr-grammar.knowledgeset/{meta.json, template.json, knowledge.db}
  - entries：uid = "kr-grammar:" + sha1(headword)[:12]；headword 冲突确定性加后缀 -2/-3 并记映射
  - entry_detail.content = view-ready JSON（键形 = src/lib/grammar/adapter.js toDetailView 输出，
    保证迁移后前端视觉零变化）：{fields, lists, related}；related.pending 键用 targetText（UI 消费键）
  - knowledge.db DDL 见 §5.3；PRAGMA user_version=1
  - 幂等：目标已存在且自检通过 → no-op 输出报告；--force 重建
  - 结束后自检对照 §5.6，输出机器可读报告 {ok, errors[], warnings[], ...}

用法：
  python3 scripts/migrate-grammar-to-kset.py                 # 默认源/目标
  python3 scripts/migrate-grammar-to-kset.py --out /tmp/k    # 自定义输出根
  python3 scripts/migrate-grammar-to-kset.py --force         # 覆盖重建
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import sqlite3
import sys
from pathlib import Path

SET_ID = "kr-grammar"
PACKAGE_NAME = f"{SET_ID}.knowledgeset"
SCHEMA_VERSION = 1
COLOR = "#D97706"
NAME = "韩语语法"
LANGUAGE = "ko"
KIND = "grammar"
DESCRIPTION = "韩语语法 · TOPIK I/II 分级 · 642 条真实语法点"
LEVELS = {
    "system": "topik",
    "label": "TOPIK",
    "values": [
        {"code": "I", "label": "TOPIK I", "rank": 1},
        {"code": "II", "label": "TOPIK II", "rank": 2},
    ],
}
TYPES = [{"value": "grammar", "label": "语法"}]

REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_SRC = REPO_ROOT / "data" / "grammar" / "grammar.db"
DEFAULT_OUT = REPO_ROOT / "src-tauri" / "resources" / "knowledge"
REGISTRY = REPO_ROOT / "src-tauri" / "resources" / "grammar" / "template_registry.json"
APPDATA_KNOWLEDGE = (
    Path.home() / "Library" / "Application Support" / "com.william.knoasis" / "knowledge"
)

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

FIELD_KEYS = [
    "pattern", "variants", "form_rule", "attaches_to", "speech_style",
    "politeness", "register", "irregular", "difficulty", "frequency",
    "meaning_brief", "usage_scene", "notes",
]
LIST_KEYS = ["meanings", "usages", "scenes", "examples", "cautions", "collocations"]
CONTENT_TOP_KEYS = {"fields", "lists", "paragraphs", "images", "related"}


def eprint(*a):
    print(*a, file=sys.stderr)


def derive_uid(headword: str) -> str:
    h = hashlib.sha1(headword.encode("utf-8")).hexdigest()[:12]
    return f"{SET_ID}:{h}"


def parse_string_array(raw):
    """对齐 Rust parse_string_array：JSON 数组优先，失败按顿号/逗号切分"""
    if raw is None:
        return []
    raw = str(raw).strip()
    if not raw:
        return []
    try:
        v = json.loads(raw)
        if isinstance(v, list):
            return [s for s in v if str(s).strip()]
    except Exception:
        pass
    return [
        s.strip()
        for s in re.split(r"[、,，;；]", raw)
        if s.strip()
    ]


def s_or_empty(v) -> str:
    return "" if v is None else str(v)


def split_tags_source(tags_text):
    """对齐 Rust split_tags 对 tags_text 的分割（供 entry_tags 缺失时回退）"""
    out, seen = [], set()
    if not tags_text:
        return out
    for part in re.split(r"[,，、;；|]", tags_text):
        t = part.strip()
        if t and t not in seen:
            seen.add(t)
            out.append(t)
    return out


def collect_tags(cur, entry_id: int, tags_text) -> list:
    """标签：entry_tags 规范行为主 + tags_text 兜底；保序去重（对齐旧 list_entries 的 tag chips）"""
    tags = split_tags_source(tags_text)
    seen = set(tags)
    rows = cur.execute(
        "SELECT tag FROM entry_tags WHERE entry_id=? ORDER BY id", (entry_id,)
    ).fetchall()
    for (tag,) in rows:
        t = tag.strip()
        if t and t not in seen:
            seen.add(t)
            tags.append(t)
    return tags


def build_content(cur, entry_id: int, detail_row: tuple) -> dict:
    """组装修饰为 toDetailView 输出形状的 view-ready JSON。

    detail_row 列顺序对应旧 kr_grammar_detail：
    pattern, variants, form_rule, attaches_to, speech_style, politeness, register,
    irregular, difficulty, frequency, meaning_brief, usage_scene, notes
    """
    (
        pattern, variants, form_rule, attaches_to, speech_style, politeness, register,
        irregular, difficulty, frequency, meaning_brief, usage_scene, notes,
    ) = detail_row

    fields = {
        "pattern": s_or_empty(pattern),
        "variants": parse_string_array(variants),
        "form_rule": s_or_empty(form_rule),
        "attaches_to": parse_string_array(attaches_to),
        "speech_style": s_or_empty(speech_style),
        "politeness": s_or_empty(politeness),
        "register": s_or_empty(register),
        "irregular": bool(irregular),
        "difficulty": None if difficulty is None else int(difficulty),
        "frequency": s_or_empty(frequency),
        "meaning_brief": s_or_empty(meaning_brief),
        "usage_scene": s_or_empty(usage_scene),
        "notes": s_or_empty(notes),
    }

    # 多值子表（子表 TEXT 列大量为 NULL → COALESCE('') 读取）
    def q_rows(sql, mapper):
        rows = cur.execute(sql, (entry_id,)).fetchall()
        return [mapper(r) for r in rows]

    meanings = q_rows(
        "SELECT COALESCE(meaning_zh,''), COALESCE(meaning_ko,''), COALESCE(note,'') "
        "FROM kr_grammar_meanings WHERE entry_id=? ORDER BY ord, id",
        lambda r: {"meaning_zh": r[0], "meaning_ko": r[1], "note": r[2]},
    )
    usages = q_rows(
        "SELECT COALESCE(title,''), COALESCE(usage_zh,''), COALESCE(usage_ko,''), COALESCE(form,'') "
        "FROM kr_grammar_usages WHERE entry_id=? ORDER BY ord, id",
        lambda r: {"title": r[0], "usage_zh": r[1], "usage_ko": r[2], "form": r[3]},
    )
    scenes = q_rows(
        "SELECT COALESCE(scene,''), COALESCE(note,'') "
        "FROM kr_grammar_scenes WHERE entry_id=? ORDER BY ord, id",
        lambda r: {"scene": r[0], "note": r[1]},
    )
    examples = q_rows(
        "SELECT COALESCE(ko,''), COALESCE(zh,''), COALESCE(note,'') "
        "FROM kr_grammar_examples WHERE entry_id=? ORDER BY ord, id",
        lambda r: {"ko": r[0], "zh": r[1], "note": r[2]},
    )
    cautions = q_rows(
        "SELECT COALESCE(caution,''), COALESCE(wrong,''), COALESCE(right,'') "
        "FROM kr_grammar_cautions WHERE entry_id=? ORDER BY ord, id",
        lambda r: {"caution": r[0], "wrong": r[1], "right": r[2]},
    )
    collocations = q_rows(
        "SELECT COALESCE(word,''), COALESCE(note,'') "
        "FROM kr_grammar_collocations WHERE entry_id=? ORDER BY ord, id",
        lambda r: {"word": r[0], "note": r[1]},
    )

    lists = {
        "meanings": meanings,
        "usages": usages,
        "scenes": scenes,
        "examples": examples,
        "cautions": cautions,
        "collocations": collocations,
    }

    related = {"resolved": [], "pending": []}
    return {
        "fields": fields,
        "lists": lists,
        "related": related,
    }


def related_for_entry(cur, entry_id: int, uid_by_headword: dict) -> dict:
    """related：resolved（目标 headword→新 uid，经映射表）+ pending（targetText 键，UI 消费）"""
    resolved = []
    rows = cur.execute(
        "SELECT r.relation, COALESCE(r.note,''), t.headword "
        "FROM kr_grammar_related r JOIN entries t ON t.id = r.related_id "
        "WHERE r.entry_id=? ORDER BY r.relation, r.id",
        (entry_id,),
    ).fetchall()
    for relation, note, headword in rows:
        resolved.append({
            "uid": uid_by_headword.get(headword, derive_uid(headword)),
            "headword": headword,
            "relation": s_or_empty(relation),
            "note": s_or_empty(note),
        })

    pending = []
    # 源列名为 kr_grammar_related_pending.target_text；content JSON 输出键固定用
    # targetText（前端 DetailRelatedBlock / export.js 消费键，见 §5.4 示例）。
    rows = cur.execute(
        "SELECT target_text, COALESCE(relation,''), COALESCE(note,'') "
        "FROM kr_grammar_related_pending WHERE entry_id=? ORDER BY id",
        (entry_id,),
    ).fetchall()
    for target_text, relation, note in rows:
        pending.append({
            "targetText": s_or_empty(target_text),
            "relation": s_or_empty(relation),
            "note": s_or_empty(note),
        })
    return {"resolved": resolved, "pending": pending}


def load_template_json() -> dict:
    """由旧 template_registry 的 kr_grammar 段原样迁出"""
    if not REGISTRY.is_file():
        raise SystemExit(f"缺少 template_registry.json: {REGISTRY}")
    registry = json.loads(REGISTRY.read_text(encoding="utf-8"))
    seg = registry.get("kr_grammar")
    if not isinstance(seg, dict):
        raise SystemExit("template_registry.json 缺少 kr_grammar 段")
    return seg


def build_meta_json(entry_count: int) -> dict:
    return {
        "schema_version": SCHEMA_VERSION,
        "id": SET_ID,
        "name": NAME,
        "version": "1.0.0",
        "language": LANGUAGE,
        "kind": KIND,
        "description": DESCRIPTION,
        "author": {"name": "William / Knoasis"},
        "homepage": "",
        "license": "proprietary",
        "color": COLOR,
        "levels": LEVELS,
        "types": TYPES,
        "entry_count": entry_count,
        "update_url": None,
    }


def source_entry_rows(src):
    cur = src.cursor()
    subjects = {r[0] for r in cur.execute("SELECT code FROM subjects")}
    if "kr_grammar" not in subjects:
        raise SystemExit("源库 subjects 中缺少 kr_grammar")
    return cur


def make_knowledge_db(target_dir: Path, src: sqlite3.Connection) -> dict:
    """迁移主体。返回统计 {entry_count, conflict_count, uid_mapping_count}"""
    cur = src.cursor()

    # 先收集 headword→uid（含冲突确定性后缀），供 related.resolved 映射
    rows = cur.execute(
        "SELECT id, headword FROM entries WHERE subject_code='kr_grammar' ORDER BY id"
    ).fetchall()
    uid_by_headword: dict = {}
    used_uids: set = set()
    used_headwords: set = set()
    mapping = {}  # 原始 headword -> 实际写入 headword（冲突加后缀）
    conflicts = 0

    for eid, headword in rows:
        final_hw = headword
        if final_hw in used_headwords:
            # 源库 UNIQUE(subject_code, headword) 理论不会触发；防御性确定性加后缀
            n = 2
            while f"{final_hw}-{n}" in used_headwords:
                n += 1
            final_hw = f"{headword}-{n}"
            conflicts += 1
        uid = derive_uid(final_hw)
        n = 2
        while uid in used_uids:
            # 极低概率 sha1 前缀撞车：再对 headword 加后缀重派生
            final_hw = f"{final_hw}-{n}"
            uid = derive_uid(final_hw)
            conflicts += 1
            n += 1
        used_headwords.add(final_hw)
        used_uids.add(uid)
        uid_by_headword[headword] = uid
        mapping[headword] = final_hw

    detail_cols = (
        "pattern, variants, form_rule, attaches_to, speech_style, politeness, register, "
        "irregular, difficulty, frequency, meaning_brief, usage_scene, notes"
    )
    detail_sql = f"SELECT {detail_cols} FROM kr_grammar_detail WHERE entry_id=?"

    db_path = target_dir / "knowledge.db"
    dst = sqlite3.connect(str(db_path))
    dst.executescript(DDL)

    entry_count = 0
    detail_count = 0
    for eid, headword in rows:
        final_hw = mapping[headword]
        uid = uid_by_headword[headword]
        meta_row = cur.execute(
            "SELECT category, level, tags_text FROM entries WHERE id=?", (eid,)
        ).fetchone()
        category = s_or_empty(meta_row[0])
        level = s_or_empty(meta_row[1])
        tags_text = meta_row[2]
        tags = ",".join(collect_tags(cur, eid, tags_text))

        drow = cur.execute(detail_sql, (eid,)).fetchone()
        summary = s_or_empty(drow[10]) if drow else ""  # meaning_brief 列（0-based 10）
        content = build_content(cur, eid, drow)
        content["related"] = related_for_entry(cur, eid, uid_by_headword)

        dst.execute(
            "INSERT INTO entries(id, uid, headword, category, level, tags, summary) "
            "VALUES (?,?,?,?,?,?,?)",
            (entry_count + 1, uid, final_hw, category, level, tags, summary),
        )
        dst.execute(
            "INSERT INTO entry_detail(entry_id, content) VALUES (?,?)",
            (entry_count + 1, json.dumps(content, ensure_ascii=False, separators=(",", ":"))),
        )
        entry_count += 1
        detail_count += 1

    dst.commit()
    dst.close()
    return {
        "entry_count": entry_count,
        "conflict_count": conflicts,
        "uid_mapping_count": len(uid_by_headword),
    }


# ---------------------------------------------------------------------------
# 自检（对照 §5.6 校验清单）
# ---------------------------------------------------------------------------

def run_selfcheck(pkg_dir: Path) -> dict:
    errors, warnings = [], []
    meta_path = pkg_dir / "meta.json"
    db_path = pkg_dir / "knowledge.db"
    template_path = pkg_dir / "template.json"

    ok = True
    meta = {}
    entry_count = 0
    try:
        meta = json.loads(meta_path.read_text(encoding="utf-8"))
        if meta.get("id") != SET_ID:
            errors.append(f"meta.id={meta.get('id')} != {SET_ID}")
        if int(meta.get("schema_version", -1)) > SCHEMA_VERSION:
            errors.append("schema_version 高于当前支持")
    except Exception as e:  # noqa: BLE001
        errors.append(f"meta.json 不可读: {e}")

    if not db_path.is_file():
        errors.append("缺少 knowledge.db")
        return _report(False, errors, warnings, meta, 0, 0, 0)

    conn = sqlite3.connect(str(db_path))
    try:
        ver = conn.execute("PRAGMA user_version").fetchone()[0]
        if ver != SCHEMA_VERSION:
            errors.append(f"PRAGMA user_version={ver} != {SCHEMA_VERSION}")

        tables = {
            r[0] for r in conn.execute(
                "SELECT name FROM sqlite_master WHERE type='table'"
            ).fetchall()
        }
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

        # uid 重算一致且唯一；headword 唯一
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

        # content 全合法 JSON + 顶层键合法
        declared_codes = {v["code"] for v in meta.get("levels", {}).get("values", [])} if meta else set()
        for (content_raw,) in conn.execute("SELECT content FROM entry_detail").fetchall():
            try:
                c = json.loads(content_raw)
            except Exception as e:  # noqa: BLE001
                errors.append(f"content 非法 JSON: {e}")
                continue
            if not isinstance(c, dict):
                errors.append("content 顶层非对象")
                continue
            extra = set(c.keys()) - CONTENT_TOP_KEYS
            if extra:
                errors.append(f"content 含未允许顶层键: {sorted(extra)}")
            if "fields" in c and not isinstance(c["fields"], dict):
                errors.append("fields 非对象")
            if "lists" in c and not isinstance(c["lists"], dict):
                errors.append("lists 非对象")
            for rel in (c.get("images") or []):
                rel_str = rel.get("rel") if isinstance(rel, dict) else rel
                if rel_str and (".." in str(rel_str) or str(rel_str).startswith("/")):
                    errors.append(f"images.rel 越界: {rel_str}")
        # level 受控词表（'' 未分级允许）
        if meta:
            for (lv,) in conn.execute(
                "SELECT DISTINCT level FROM entries WHERE level IS NOT NULL AND level<>''"
            ).fetchall():
                if lv not in declared_codes:
                    warnings.append(f"level {lv} 不在 meta.levels 声明内")
    finally:
        conn.close()

    ok = not errors
    return _report(ok, errors, warnings, meta, entry_count, entry_count, 0)


def _report(ok, errors, warnings, meta, entry_count, uid_mapping_count, conflict_count):
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
        "uid_mapping_count": uid_mapping_count,
        "conflict_count": conflict_count,
    }


def sync_to_appdata(pkg_dir: Path) -> Path:
    dest = APPDATA_KNOWLEDGE / PACKAGE_NAME
    dest.parent.mkdir(parents=True, exist_ok=True)
    if dest.exists():
        shutil.rmtree(dest)
    shutil.copytree(pkg_dir, dest)
    return dest


def main() -> int:
    ap = argparse.ArgumentParser(description="迁移 kr grammar → kr-grammar.knowledgeset")
    ap.add_argument("--src", type=Path, default=DEFAULT_SRC)
    ap.add_argument("--out", type=Path, default=DEFAULT_OUT)
    ap.add_argument("--force", action="store_true", help="覆盖重建")
    ap.add_argument("--no-sync", action="store_true", help="不复制到 APPDATA 用户根")
    args = ap.parse_args()

    src = Path(args.src)
    out_root = Path(args.out)
    if not src.is_file():
        eprint(f"源库不存在: {src}")
        return 1

    pkg_dir = out_root / PACKAGE_NAME
    src_conn = sqlite3.connect(f"file:{src}?mode=ro", uri=True)
    src_conn.row_factory = sqlite3.Row
    try:
        source_entry_rows(src_conn)
    finally:
        src_conn.close()

    # 幂等：目标存在且自检通过 → no-op；否则需 --force
    if pkg_dir.exists():
        report = run_selfcheck(pkg_dir)
        if report["ok"] and not args.force:
            print(json.dumps(report, ensure_ascii=False, indent=2))
            if not args.no_sync:
                dest = sync_to_appdata(pkg_dir)
                print(f"[sync] APPDATA 已同步: {dest}")
            print("[status] SKIP（目标已存在且校验通过；--force 可重建）")
            return 0
        if not report["ok"] and not args.force:
            eprint("目标已存在但校验未通过：需 --force 覆盖重建")
            eprint(json.dumps(report, ensure_ascii=False, indent=2))
            return 2

    if pkg_dir.exists():
        shutil.rmtree(pkg_dir)
    pkg_dir.mkdir(parents=True, exist_ok=True)

    # 迁移主体
    src_conn = sqlite3.connect(f"file:{src}?mode=ro", uri=True)
    try:
        stats = make_knowledge_db(pkg_dir, src_conn)
    finally:
        src_conn.close()

    # meta.json + template.json
    (pkg_dir / "meta.json").write_text(
        json.dumps(build_meta_json(stats["entry_count"]), ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    (pkg_dir / "template.json").write_text(
        json.dumps(load_template_json(), ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )

    report = run_selfcheck(pkg_dir)
    report["uid_mapping_count"] = stats["uid_mapping_count"]
    report["conflict_count"] = stats["conflict_count"]
    print(json.dumps(report, ensure_ascii=False, indent=2))

    if not report["ok"]:
        eprint("迁移自检未通过，检查 errors。")
        return 3

    if not args.no_sync:
        dest = sync_to_appdata(pkg_dir)
        print(f"[sync] APPDATA 已同步: {dest}")
    print(f"[done] 包已产出: {pkg_dir}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
