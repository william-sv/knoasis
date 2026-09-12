#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Knoasis · 英语语法 v3 数据 → en-grammar.knowledgeset 学科包（打包工具）

设计：docs/Knoasis-学科数据组织与第三方接入方案.md §5（契约 v1）+ QA 审计定案。
  - 源 = en_grammar_v3.json（顶层 {schema, version, node_count, nodes}；22 节点；
    每 sub_topic 恒为 {topic, explanation_zh, examples:[{en, zh}]}，共 167 条）
  - 目标包：en-grammar.knowledgeset/{meta.json, template.json, knowledge.db}
  - entries：headword=topic / category=节点名 / level=''（空串，未分级）/ tags=节点名 /
    summary=explanation_zh 首句（超 120 字按句末标点截断，无标点硬截）
    uid = "en-grammar:" + sha1(headword 原始 UTF-8)[:12]（冲突确定性加后缀 -2/-3 并记 warning）
  - entry_detail.content = view-ready JSON：
    {"paragraphs":{"explanation": <全文>}, "lists":{"examples":[{"en":..,"zh":..}]}}
    （键名严格 paragraphs.explanation / lists.examples；explanation_zh 为空 → 跳过记 error；
      examples 为空数组 → 省略 lists.examples 键；不写 related）
  - 引号规范化：explanation_zh 中成对 ASCII 双引号 "…" → 弯引号 “…”/”…”；
    绝不动单引号撇号（don't / 's / 've 是英文单词一部分）。只处理双引号。
  - knowledge.db DDL 与 kr-grammar.knowledgeset 实物 sqlite_master 完全一致；
    PRAGMA user_version=1
  - 幂等：目标存在且自检通过 → no-op 输出报告；--force 重建
  - 结束后自检对照 §5.6，输出机器可读报告 {ok, errors[], warnings[], ...}；
    并把包同步到 $APPDATA/com.william.knoasis/knowledge/（用户根，整目录覆盖语义）

用法：
  python3 scripts/import-en-v3-to-kset.py                     # 默认源/目标
  python3 scripts/import-en-v3-to-kset.py --src /path/en.json # 指定数据源
  python3 scripts/import-en-v3-to-kset.py --out /tmp/k        # 自定义输出根
  python3 scripts/import-en-v3-to-kset.py --force             # 覆盖重建
  python3 scripts/import-en-v3-to-kset.py --no-sync           # 不同步 APPDATA 用户根
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

SET_ID = "en-grammar"
PACKAGE_NAME = f"{SET_ID}.knowledgeset"
SCHEMA_VERSION = 1
COLOR = "#2563EB"
NAME = "英语语法"
LANGUAGE = "en"
KIND = "grammar"
VERSION = "1.0.0"
DESCRIPTION = "英语语法 · 词法到句法系统讲解 · 167 条（中文讲解 + 双语例句）"
TYPES = [{"value": "grammar", "label": "语法"}]
# v3 无难度字段 → meta.json 不写 levels；entries.level 一律空串（UI 归"未分级"）

REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_SRC = Path(
    os.environ.get("EN_V3_SRC", "/Users/william/Downloads/yufa/out/en_grammar_v3.json")
)
DEFAULT_OUT = REPO_ROOT / "src-tauri" / "resources" / "knowledge"
APPDATA_KNOWLEDGE = (
    Path.home() / "Library" / "Application Support" / "com.william.knoasis" / "knowledge"
)

# 与 kr-grammar.knowledgeset/knowledge.db sqlite_master 实物完全一致（契约 §5.3 DDL）
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

TEMPLATE = {
    "label": NAME,
    "headwordLabel": "语法点",
    "sections": [
        {"key": "explanation", "label": "讲解", "type": "paragraph"},
        {
            "key": "examples",
            "label": "例句（双语）",
            "type": "list",
            "itemFields": [
                {"key": "en", "label": "英文"},
                {"key": "zh", "label": "中文"},
            ],
        },
    ],
}

SENT_END = "。！？"
SUMMARY_MAXLEN = 120
CONTENT_TOP_KEYS = {"paragraphs", "lists"}


def eprint(*a):
    print(*a, file=sys.stderr)


def derive_uid(headword: str) -> str:
    """uid = "{set_id}:{sha1(headword UTF-8) hex[:12]}"（与 kr 包同算法）"""
    h = hashlib.sha1(headword.encode("utf-8")).hexdigest()[:12]
    return f"{SET_ID}:{h}"


def normalize_quotes(text: str) -> tuple[str, int, int]:
    """把成对 ASCII 双引号 "…" 替换为弯引号 “…”/”…”；返回 (新文本, 成对数, 落单数)。

    只处理双引号；单引号撇号（don't、's、've 等英文单词一部分）一律不动。
    按出现顺序成对配对：第奇数次 → “，第偶数次 → ”。源数据经 QA 审计无落单双引号；
    若出现落单（防御），最后一只保持 ASCII 原样，避免悬空弯引号。
    """
    out = []
    pairs = 0
    unmatched = 0
    open_q = False
    pending = -1  # 当前未闭合引号在 out 中的下标；-1 表示无
    for ch in text:
        if ch == '"':
            if not open_q:
                out.append("\u201c")  # “
                open_q = True
                pending = len(out) - 1
            else:
                out.append("\u201d")  # ”
                open_q = False
                pending = -1
                pairs += 1
        else:
            out.append(ch)
    if open_q and pending >= 0:
        # 落单的开引号还原为 ASCII，避免悬空
        out[pending] = '"'
        unmatched += 1
    return "".join(out), pairs, unmatched


def summary_of(text: str, maxlen: int = SUMMARY_MAXLEN) -> str:
    """summary = explanation_zh 首句（到第一个句末标点 。！？ 为止）。

    首句 ≤ maxlen → 取完整首句；首句 > maxlen → 截到 maxlen 内最后一个句末标点；
    前 maxlen 内无句末标点 → 硬截 maxlen。
    """
    t = text.strip()
    m = re.search(rf"[{re.escape(SENT_END)}]", t)
    if m and m.end() <= maxlen:
        return t[: m.end()]
    sub = t[:maxlen]
    idx = max(sub.rfind(c) for c in SENT_END)
    if idx >= 0:
        return t[: idx + 1]
    return t[:maxlen]


def build_meta_json(entry_count: int) -> dict:
    return {
        "id": SET_ID,
        "name": NAME,
        "language": LANGUAGE,
        "kind": KIND,
        "version": VERSION,
        "schema_version": SCHEMA_VERSION,
        "color": COLOR,
        "description": DESCRIPTION,
        "types": TYPES,
        "entry_count": entry_count,
    }


def parse_source(src_path: Path) -> tuple[list, str, str, int]:
    """只读加载并校验源 JSON 顶层结构。返回 (nodes, schema, version, node_count)。"""
    with open(src_path, "r", encoding="utf-8") as f:
        data = json.load(f)
    if not isinstance(data, dict):
        raise SystemExit(f"源 JSON 顶层非对象: {src_path}")
    nodes = data.get("nodes")
    if not isinstance(nodes, list) or not nodes:
        raise SystemExit(f"源 JSON nodes 缺失或为空: {src_path}")
    if data.get("schema") != "en_grammar_v3":
        eprint(f"[warn] 源 schema={data.get('schema')!r} != 'en_grammar_v3'（继续）")
    return nodes, str(data.get("schema", "")), str(data.get("version", "")), int(data.get("node_count", -1))


def source_quote_pairs(nodes: list) -> int:
    """按源数据统计 explanation_zh 将被替换的 ASCII 双引号成对数（用于 SKIP 报告）。"""
    total = 0
    for node in nodes:
        for st in node.get("sub_topics", []):
            _, pairs, _ = normalize_quotes(str(st.get("explanation_zh", "")))
            total += pairs
    return total


def make_knowledge_db(pkg_dir: Path, nodes: list) -> dict:
    """打包主体。返回统计 {entry_count, conflict_count, quote_pairs, skipped_empty}。"""
    used_headwords: set = set()
    used_uids: set = set()
    conflicts = 0
    skipped_empty = 0
    quote_pairs = 0

    db_path = pkg_dir / "knowledge.db"
    dst = sqlite3.connect(str(db_path))
    dst.executescript(DDL)

    seq = 0
    for node in nodes:
        node_name = str(node.get("node", "")).strip()
        if not node_name:
            raise SystemExit(f"节点缺 node 名: {node}")
        for st in node.get("sub_topics", []):
            topic = str(st.get("topic", "")).strip()
            explanation = str(st.get("explanation_zh", "")).strip()
            if not topic:
                raise SystemExit(f"sub_topic 缺 topic（节点 {node_name}）")
            if not explanation:
                # QA 预期 0；防御：跳过并记 error（不影响后续，调用方以 skipped_empty 判失败）
                skipped_empty += 1
                eprint(f"[error] explanation_zh 为空，跳过: {node_name}/{topic}")
                continue

            # 引号规范化（只处理双引号；在派生 summary 之前做，保证列表卡与详情一致）
            normalized, pairs, unmatched = normalize_quotes(explanation)
            quote_pairs += pairs
            if unmatched:
                eprint(f"[warn] explanation_zh 含落单 ASCII 双引号（保持原样）: {topic}")

            summary = summary_of(normalized)
            if not summary:
                raise SystemExit(f"summary 为空: {node_name}/{topic}")

            # headword 冲突（防御；源已确认唯一）→ 确定性加后缀 -2/-3…
            final_hw = topic
            while final_hw in used_headwords:
                n = 2
                while f"{topic}-{n}" in used_headwords:
                    n += 1
                final_hw = f"{topic}-{n}"
                conflicts += 1
            uid = derive_uid(final_hw)
            while uid in used_uids:
                # 极低概率 sha1 前缀撞车：对 headword 加后缀重派生
                n = 2
                while f"{final_hw}-{n}" in used_headwords:
                    n += 1
                final_hw = f"{final_hw}-{n}"
                uid = derive_uid(final_hw)
                conflicts += 1
            used_headwords.add(final_hw)
            used_uids.add(uid)

            # content JSON：paragraphs.explanation 恒在；examples 非空才写 lists.examples
            content: dict = {"paragraphs": {"explanation": normalized}}
            raw_examples = st.get("examples")
            if isinstance(raw_examples, list) and raw_examples:
                cleaned = []
                for ex in raw_examples:
                    if not isinstance(ex, dict):
                        continue
                    en = ex.get("en")
                    zh = ex.get("zh")
                    if en is None and zh is None:
                        continue
                    cleaned.append({"en": "" if en is None else str(en),
                                    "zh": "" if zh is None else str(zh)})
                if cleaned:
                    content["lists"] = {"examples": cleaned}

            seq += 1
            dst.execute(
                "INSERT INTO entries(id, uid, headword, category, level, tags, summary) "
                "VALUES (?,?,?,?,?,?,?)",
                (seq, uid, final_hw, node_name, "", node_name, summary),
            )
            dst.execute(
                "INSERT INTO entry_detail(entry_id, content) VALUES (?,?)",
                (seq, json.dumps(content, ensure_ascii=False, separators=(",", ":"))),
            )

    dst.commit()
    dst.close()
    return {
        "entry_count": seq,
        "conflict_count": conflicts,
        "quote_pairs": quote_pairs,
        "skipped_empty": skipped_empty,
    }


# ---------------------------------------------------------------------------
# 自检（对照契约 §5.6 校验清单 + 任务定案校验项）
# ---------------------------------------------------------------------------

def run_selfcheck(pkg_dir: Path) -> dict:
    errors, warnings = [], []
    meta_path = pkg_dir / "meta.json"
    db_path = pkg_dir / "knowledge.db"

    meta = {}
    entry_count = 0
    quote_pairs = 0
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

    conn = None
    try:
        conn = sqlite3.connect(str(db_path))
        ver = conn.execute("PRAGMA user_version").fetchone()[0]
        if ver != SCHEMA_VERSION:
            errors.append(f"PRAGMA user_version={ver} != {SCHEMA_VERSION}")

        tables = {
            r[0]
            for r in conn.execute(
                "SELECT name FROM sqlite_master WHERE type='table'"
            ).fetchall()
        }
        required_tables = {"entries", "entry_detail"}
        for t in required_tables:
            if t not in tables:
                errors.append(f"缺表 {t}")
        # 列齐全（与 kr 实物 / §5.3 对齐）；表缺失时跳过列级/数据级检查避免误报级联
        expected_cols = {
            "entries": ["id", "uid", "headword", "category", "level", "tags", "summary", "sort", "extra"],
            "entry_detail": ["entry_id", "content"],
        }
        if "entries" in tables:
            got = {r[1] for r in conn.execute("PRAGMA table_info(entries)")}
            miss = set(expected_cols["entries"]) - got
            if miss:
                errors.append(f"entries 缺列: {sorted(miss)}")
        if "entry_detail" in tables:
            got = {r[1] for r in conn.execute("PRAGMA table_info(entry_detail)")}
            miss = set(expected_cols["entry_detail"]) - got
            if miss:
                errors.append(f"entry_detail 缺列: {sorted(miss)}")

        if not required_tables.issubset(tables):
            ok = not errors
            return _report(ok, errors, warnings, meta, 0, 0, 0)

        fk = conn.execute("PRAGMA foreign_key_check").fetchall()
        if fk:
            errors.append(f"foreign_key_check 悬空 {len(fk)} 条")

        entry_count = conn.execute("SELECT COUNT(*) FROM entries").fetchone()[0]
        detail_count = conn.execute("SELECT COUNT(*) FROM entry_detail").fetchone()[0]
        if entry_count != detail_count:
            errors.append(f"entries({entry_count}) != entry_detail({detail_count})")

        rows = conn.execute("SELECT uid, headword, level, summary FROM entries").fetchall()
        seen_uid, seen_hw = set(), set()
        for uid, headword, level, summary in rows:
            if derive_uid(headword) != uid:
                errors.append(f"uid 重算不一致: {headword} {uid}")
            if not re.fullmatch(r"en-grammar:[0-9a-f]{12}", uid):
                errors.append(f"uid 格式非法: {uid}")
            if uid in seen_uid:
                errors.append(f"uid 重复: {uid}")
            if headword in seen_hw:
                warnings.append(f"headword 重复: {headword}")
            if not summary:
                errors.append(f"summary 为空: {headword}")
            if level not in ("", None):
                warnings.append(f"level 非空（meta 未声明 levels）: {headword}={level!r}")
            seen_uid.add(uid)
            seen_hw.add(headword)

        # content 合法 JSON 且键形 {paragraphs:{explanation}, lists:{examples}}
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
            paragraphs = c.get("paragraphs")
            if not isinstance(paragraphs, dict) or not isinstance(paragraphs.get("explanation"), str) \
                    or not paragraphs.get("explanation"):
                errors.append("content 缺 paragraphs.explanation 非空字符串")
            lists = c.get("lists")
            if lists is not None:
                if not isinstance(lists, dict):
                    errors.append("lists 非对象")
                else:
                    ex_extra = set(lists.keys()) - {"examples"}
                    if ex_extra:
                        errors.append(f"lists 含未允许键: {sorted(ex_extra)}")
                    examples = lists.get("examples")
                    if examples is not None:
                        if not isinstance(examples, list):
                            errors.append("lists.examples 非数组")
                        else:
                            for i, ex in enumerate(examples):
                                if not isinstance(ex, dict) or not ex.get("en") or not ex.get("zh"):
                                    errors.append(f"lists.examples[{i}] 缺 en/zh 非空: {ex}")

        # level 受控词表（meta 未声明 levels → 一切非空 level 均越界，上面已报）
        # entries/entry_detail 计数与 meta.entry_count 对照
        if meta:
            mc = meta.get("entry_count")
            if mc is not None and int(mc) != entry_count:
                errors.append(f"meta.entry_count={mc} != 实际 {entry_count}")
    except sqlite3.DatabaseError as e:
        # knowledge.db 为垃圾文件/损坏：connect 或库内只读探测会抛 DatabaseError。
        # 捕获后记入 errors → main 幂等预检走既有 "需 --force 重建" exit=2 分支，
        # 保证无 traceback、exit code 正确。
        errors.append(f"knowledge.db 打开/读取失败（疑似损坏或非 SQLite 文件）: {e}")
    finally:
        if conn is not None:
            conn.close()

    ok = not errors
    return _report(ok, errors, warnings, meta, entry_count, quote_pairs, 0)


def _report(ok, errors, warnings, meta, entry_count, quote_pairs, conflict_count):
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
        "quote_pairs_replaced": quote_pairs,
        "conflict_count": conflict_count,
    }


def sync_to_appdata(pkg_dir: Path) -> Path:
    """用户根整目录覆盖语义：用户根版本应 ≥ 内置版本；同版本直接覆盖。"""
    dest = APPDATA_KNOWLEDGE / PACKAGE_NAME
    dest.parent.mkdir(parents=True, exist_ok=True)
    if dest.exists():
        shutil.rmtree(dest)
    shutil.copytree(pkg_dir, dest)
    return dest


def main() -> int:
    ap = argparse.ArgumentParser(description="打包 en_grammar_v3 → en-grammar.knowledgeset")
    ap.add_argument("--src", type=Path, default=DEFAULT_SRC, help="源 en_grammar_v3.json")
    ap.add_argument("--out", type=Path, default=DEFAULT_OUT, help="输出 knowledge 根（默认 src-tauri/resources/knowledge）")
    ap.add_argument("--force", action="store_true", help="覆盖重建")
    ap.add_argument("--no-sync", action="store_true", help="不复制到 APPDATA 用户根")
    args = ap.parse_args()

    src = Path(args.src)
    out_root = Path(args.out)
    if not src.is_file():
        eprint(f"源文件不存在: {src}")
        return 1

    pkg_dir = out_root / PACKAGE_NAME
    nodes, schema, version, node_count = parse_source(src)
    total_sub = sum(len(n.get("sub_topics", [])) for n in nodes)
    eprint(f"[src] {src.name}: schema={schema} version={version} node_count={node_count} "
           f"nodes={len(nodes)} sub_topics={total_sub}")

    # 幂等：目标存在且自检通过 → no-op；否则需 --force
    if pkg_dir.exists():
        report = run_selfcheck(pkg_dir)
        if report["ok"] and not args.force:
            # quote_pairs_replaced 是构建期统计：SKIP 时按源数据重算，保持报告语义一致
            report["quote_pairs_replaced"] = source_quote_pairs(nodes)
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

    stats = make_knowledge_db(pkg_dir, nodes)
    if stats["skipped_empty"]:
        eprint(f"[error] explanation_zh 为空被跳过 {stats['skipped_empty']} 条（QA 预期 0）")
        return 3

    # meta.json + template.json
    (pkg_dir / "meta.json").write_text(
        json.dumps(build_meta_json(stats["entry_count"]), ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    (pkg_dir / "template.json").write_text(
        json.dumps(TEMPLATE, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )

    report = run_selfcheck(pkg_dir)
    report["quote_pairs_replaced"] = stats["quote_pairs"]
    report["conflict_count"] = stats["conflict_count"]
    print(json.dumps(report, ensure_ascii=False, indent=2))

    if not report["ok"]:
        eprint("打包自检未通过，检查 errors。")
        return 3

    if not args.no_sync:
        dest = sync_to_appdata(pkg_dir)
        print(f"[sync] APPDATA 已同步: {dest}")
    print(f"[done] 包已产出: {pkg_dir}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
