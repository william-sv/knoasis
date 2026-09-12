// Knoasis · knowledge 包内 knowledge.db 只读访问（list_entries / get_entry / resolve_assets）
//
// 契约见 docs/Knoasis-学科数据组织与第三方接入方案.md §5.3/§5.4/§5.5。
//   - 每命令新建只读连接（SQLITE_OPEN_READ_ONLY | SQLITE_OPEN_NO_MUTEX），不持有长锁
//   - entries / entry_detail 固定语义表名；列表查询 + 详情 content JSON 单列
//   - uid 由 set_id + headword 派生；images[].rel 相对包 assets/ 根解析为绝对路径，
//     缺失文件静默跳过，并按 userdata image_hidden 过滤（隐藏过滤位置 = Rust 读出口）

use crate::grammar::db::{ApiError, CmdResult};
use crate::knowledge::{AssetImage, EntryItem, EntryPayload, KnowledgeSetMeta, ListEntriesResult, SubjectCounts};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use std::collections::HashSet;
use std::path::Path;

/// 每命令新建只读连接
pub fn open_ro(path: &Path) -> CmdResult<Connection> {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| ApiError::db_unavailable(format!("无法打开 knowledge.db：{e}")))
}

fn schema_error(msg: impl Into<String>) -> ApiError {
    ApiError::schema_mismatch(msg)
}

/// 校验包内 schema：必填表 entries/entry_detail 存在且关键列齐全。
fn ensure_schema(conn: &Connection) -> CmdResult<()> {
    let need_tables = ["entries", "entry_detail"];
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table'")
        .map_err(|e| schema_error(format!("读 sqlite_master 失败：{e}")))?;
    let names: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| schema_error(format!("读 sqlite_master 失败：{e}")))?
        .collect::<Result<_, _>>()
        .map_err(|e| schema_error(format!("读 sqlite_master 失败：{e}")))?;
    for t in need_tables {
        if !names.iter().any(|n| n == t) {
            return Err(schema_error(format!("knowledge.db 缺表 {t}：包结构不符 v1 契约")));
        }
    }
    // 列校验（SELECT LIMIT 0 探针足够）
    let checks: &[(&str, &str)] = &[
        ("entries", "uid"),
        ("entries", "headword"),
        ("entries", "category"),
        ("entries", "level"),
        ("entries", "tags"),
        ("entries", "summary"),
        ("entry_detail", "entry_id"),
        ("entry_detail", "content"),
    ];
    for (t, c) in checks {
        conn.prepare(&format!("SELECT {c} FROM {t} LIMIT 0"))
            .map_err(|_| {
                schema_error(format!("knowledge.db 表 {t} 缺列 {c}：包结构不符 v1 契约"))
            })?;
    }
    Ok(())
}

/// 打开并校验（返回连接）
pub fn open_and_validate(path: &Path) -> CmdResult<Connection> {
    let conn = open_ro(path)?;
    ensure_schema(&conn)?;
    Ok(conn)
}

/// 由 db 路径直接算 counts（discovery 扫描用；失败给人类可读 Err）
pub fn counts_from_db(path: &Path) -> Result<SubjectCounts, String> {
    let conn = open_ro(path).map_err(|e| e.message)?;
    ensure_schema(&conn).map_err(|e| e.message)?;
    counts(&conn).map_err(|e| e.message)
}

/// total + by_level（含 '' 未分级桶，与既有 grammar SubjectCounts 形状一致）
pub fn counts(conn: &Connection) -> CmdResult<SubjectCounts> {
    let total: i64 = conn
        .query_row("SELECT COUNT(*) FROM entries", [], |r| r.get(0))
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut by_level = std::collections::BTreeMap::new();
    let mut stmt = conn
        .prepare("SELECT COALESCE(level,''), COUNT(*) FROM entries GROUP BY COALESCE(level,'')")
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
        .map_err(|e| ApiError::internal(e.to_string()))?;
    for row in rows {
        let (lv, n) = row.map_err(|e| ApiError::internal(e.to_string()))?;
        by_level.insert(lv, n as usize);
    }
    Ok(SubjectCounts {
        total: total as usize,
        by_level,
    })
}

/// 解析 tags 列：按逗号/顿号/分号/竖线拆分 + 去重保序（与旧 grammar split_tags 兼容）
fn split_tags(tags_text: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out: Vec<String> = Vec::new();
    for part in tags_text.split(|c| matches!(c, ',' | '，' | '、' | ';' | '；' | '|')) {
        let t = part.trim();
        if t.is_empty() || seen.contains(t) {
            continue;
        }
        seen.insert(t.to_string());
        out.push(t.to_string());
    }
    out
}

/// code → (label, rank)。空 code → 未分级；命中 meta.levels 用包声明；未命中回落 code 原样。
pub fn level_meta(meta: &KnowledgeSetMeta, code: &str) -> (String, u32) {
    if code.is_empty() {
        return ("未分级".to_string(), 0);
    }
    for lv in &meta.levels {
        if lv.code == code {
            return (lv.label.clone(), lv.rank);
        }
    }
    (code.to_string(), 0)
}

/// 列表查询过滤器
#[derive(Debug, Default, Clone)]
pub struct EntryFilter<'a> {
    pub level: Option<&'a str>,
    pub category: Option<&'a str>,
    pub q: Option<&'a str>,
    pub limit: u32,
}

fn push_where(
    sql: &mut String,
    pv: &mut Vec<rusqlite::types::Value>,
    cond: &str,
    val: rusqlite::types::Value,
) {
    sql.push_str(cond);
    pv.push(val);
}

fn build_where(f: &EntryFilter<'_>) -> (String, Vec<rusqlite::types::Value>) {
    let mut sql = String::from(" WHERE 1=1");
    let mut pv: Vec<rusqlite::types::Value> = Vec::new();
    if let Some(lv) = f.level.filter(|s| !s.is_empty()) {
        push_where(&mut sql, &mut pv, " AND COALESCE(level,'') = ?", lv.to_string().into());
    }
    if let Some(cat) = f.category.filter(|s| !s.is_empty()) {
        push_where(
            &mut sql,
            &mut pv,
            " AND COALESCE(category,'') = ?",
            cat.to_string().into(),
        );
    }
    if let Some(q) = f.q.map(|s| s.trim()).filter(|s| !s.is_empty()) {
        let like = format!("%{q}%");
        sql.push_str(
            " AND (headword LIKE ? OR COALESCE(tags,'') LIKE ? OR COALESCE(summary,'') LIKE ?)",
        );
        pv.push(like.clone().into());
        pv.push(like.clone().into());
        pv.push(like.into());
    }
    (sql, pv)
}

/// 通用 entries 查询（level/category/q 过滤 + 总数 + 分页 limit）
pub fn list_entries(meta: &KnowledgeSetMeta, f: &EntryFilter<'_>) -> CmdResult<ListEntriesResult> {
    let conn = open_and_validate(&meta.db_path)?;
    let (where_sql, pv) = build_where(f);

    // 总数（不限 limit）
    let total: i64 = {
        let sql = format!("SELECT COUNT(*) FROM entries{where_sql}");
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| ApiError::internal(e.to_string()))?;
        stmt.query_row(rusqlite::params_from_iter(pv.iter()), |r| r.get(0))
            .map_err(|e| ApiError::internal(e.to_string()))?
    };

    let limit = f.limit.max(1) as i64;
    let mut sql = String::from(
        "SELECT uid, headword, COALESCE(category,''), COALESCE(level,''), COALESCE(tags,''), COALESCE(summary,'') \
         FROM entries",
    );
    sql.push_str(&where_sql);
    sql.push_str(" ORDER BY COALESCE(category,''), headword, id LIMIT ?");
    let mut qpv = pv.clone();
    qpv.push(limit.into());

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(qpv.iter()), |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
            ))
        })
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut items: Vec<EntryItem> = Vec::new();
    for row in rows {
        let (uid_, headword, category, lv, tags_text, summary) =
            row.map_err(|e| ApiError::internal(e.to_string()))?;
        let (level_label, _rank) = level_meta(meta, &lv);
        items.push(EntryItem {
            uid: uid_,
            headword,
            category,
            level_code: lv,
            level_label,
            tags: split_tags(&tags_text),
            summary,
        });
    }
    Ok(ListEntriesResult {
        total: total as usize,
        items,
    })
}

/// 资源路径防御：拒绝绝对路径 / `..` 越界（§5.5）
fn rel_is_safe(rel: &str) -> bool {
    let rel = rel.trim();
    !rel.is_empty()
        && !rel.starts_with('/')
        && !rel.starts_with("\\")
        && !rel.split('/').any(|seg| seg == "..")
}

/// 把 rel 列表 join 包 assets/ 根解析为绝对路径；缺文件静默跳过；hidden 过滤。
/// assets_dir 为 None（包无 assets/）→ 返回空。
pub fn resolve_assets(
    assets_dir: Option<&Path>,
    rels: &[String],
    hidden: &HashSet<String>,
) -> Vec<AssetImage> {
    let Some(dir) = assets_dir else {
        return Vec::new();
    };
    rels.iter()
        .filter(|rel| rel_is_safe(rel) && !hidden.contains(rel.trim()))
        .filter_map(|rel| {
            let p = dir.join(rel.trim());
            if p.is_file() {
                Some(AssetImage {
                    rel: rel.trim().to_string(),
                    path: p.to_string_lossy().into_owned(),
                })
            } else {
                None
            }
        })
        .collect()
}

/// 从 content JSON 的 images 数组取 rel 列表（元素可为 {rel} 或字符串，v1 宽松兼容）
fn content_image_rels(content: &serde_json::Value) -> Vec<String> {
    let Some(arr) = content.get("images").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|item| {
            if let Some(s) = item.as_str() {
                Some(s.to_string())
            } else {
                item.get("rel").and_then(|r| r.as_str()).map(|r| r.to_string())
            }
        })
        .collect()
}

/// 详情：entries 行 + entry_detail.content JSON → EntryPayload；images 已解析并过滤。
pub fn get_entry(
    meta: &KnowledgeSetMeta,
    uid_str: &str,
    hidden: &HashSet<String>,
) -> CmdResult<EntryPayload> {
    let conn = open_and_validate(&meta.db_path)?;
    let row = conn
        .query_row(
            "SELECT e.uid, e.headword, COALESCE(e.category,''), COALESCE(e.level,''), \
             COALESCE(e.tags,''), COALESCE(e.summary,''), d.content \
             FROM entries e LEFT JOIN entry_detail d ON d.entry_id = e.id \
             WHERE e.uid = ?1",
            params![uid_str],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, Option<String>>(6)?,
                ))
            },
        )
        .optional()
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let Some((uid_, headword, category, lv, tags_text, summary, content_json)) = row else {
        return Err(ApiError::entry_not_found(uid_str));
    };

    let content: serde_json::Value = match content_json {
        Some(raw) => serde_json::from_str(&raw).map_err(|e| {
            schema_error(format!("条目 {headword} content 非法 JSON: {e}"))
        })?,
        None => {
            return Err(schema_error(format!("条目 {headword} 缺少 entry_detail.content 行")));
        }
    };

    let rels = content_image_rels(&content);
    let images = resolve_assets(meta.assets_dir.as_deref(), &rels, hidden);
    let (level_label, _rank) = level_meta(meta, &lv);

    Ok(EntryPayload {
        uid: uid_,
        set_id: meta.set_id.clone(),
        headword,
        category,
        level_code: lv,
        level_label,
        tags: split_tags(&tags_text),
        summary,
        content,
        images,
    })
}

// ---------------------------------------------------------------------------
// 单元测试（纯 SQL / 纯路径语义；不依赖 tauri）
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::meta::parse_meta_json;
    use crate::knowledge::uid;
    use crate::knowledge::{LevelOpt, SetOrigin, TypeOpt};
    use std::fs;

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "knoasis_reader_{}_{tag}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn make_meta(dir: &std::path::Path) -> KnowledgeSetMeta {
        let parsed = parse_meta_json(
            r#"{
              "schema_version": 1, "id": "kr-grammar", "name": "韩语语法",
              "version": "1.0.0", "language": "ko", "kind": "grammar",
              "description": "测试",
              "levels": { "system": "topik", "label": "TOPIK", "values": [
                { "code": "I", "label": "TOPIK I", "rank": 1 },
                { "code": "II", "label": "TOPIK II", "rank": 2 } ] },
              "types": [ { "value": "grammar", "label": "语法" } ]
            }"#,
        )
        .unwrap();
        let assets_dir = dir.join("assets");
        let _ = fs::create_dir_all(&assets_dir);
        fs::write(assets_dir.join("a.jpeg"), b"x").unwrap();
        KnowledgeSetMeta {
            set_id: parsed.set_id.clone(),
            name: parsed.name.clone(),
            version: parsed.version.clone(),
            schema_version: parsed.schema_version,
            language: parsed.language.clone(),
            kind: parsed.kind.clone(),
            color: parsed.color.clone(),
            description: parsed.description.clone(),
            level_system: parsed.level_system.clone(),
            level_system_label: parsed.level_system_label.clone(),
            levels: parsed.levels.clone(),
            types: parsed.types.clone(),
            template: serde_json::json!({}),
            counts: SubjectCounts::default(),
            origin: SetOrigin::Builtin,
            db_path: dir.join("knowledge.db"),
            assets_dir: Some(assets_dir),
        }
    }

    fn seed_db(dir: &std::path::Path) {
        let conn = Connection::open(dir.join("knowledge.db")).unwrap();
        conn.execute_batch(
            "CREATE TABLE entries(
               id INTEGER PRIMARY KEY,
               uid TEXT NOT NULL UNIQUE,
               headword TEXT NOT NULL UNIQUE,
               category TEXT,
               level TEXT,
               tags TEXT,
               summary TEXT,
               sort TEXT,
               extra TEXT);
             CREATE TABLE entry_detail(
               entry_id INTEGER PRIMARY KEY REFERENCES entries(id) ON DELETE CASCADE,
               content TEXT NOT NULL);",
        )
        .unwrap();
        let rows = [
            ("-고", "连接语尾", "I", "连接语尾,接续", "s1"),
            ("N마저", "助词", "II", "助词,TOPIK", "s2"),
            ("A/V-아/어요", "终结语尾", "", "", "s3"),
        ];
        for (i, (hw, cat, lv, tags, summary)) in rows.iter().enumerate() {
            conn.execute(
                "INSERT INTO entries(id, uid, headword, category, level, tags, summary) \
                 VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![
                    (i + 1) as i64,
                    uid::derive("kr-grammar", hw),
                    hw,
                    cat,
                    lv,
                    tags,
                    summary
                ],
            )
            .unwrap();
            let content = serde_json::json!({
                "fields": { "pattern": hw },
                "lists": {},
                "related": { "resolved": [], "pending": [] },
                "images": [ { "rel": "a.jpeg" }, { "rel": "missing.jpeg" } ]
            });
            conn.execute(
                "INSERT INTO entry_detail(entry_id, content) VALUES(?1,?2)",
                params![(i + 1) as i64, serde_json::to_string(&content).unwrap()],
            )
            .unwrap();
        }
    }

    #[test]
    fn list_entries_filters_and_level_labels() {
        let dir = temp_dir("list");
        seed_db(&dir);
        let meta = make_meta(&dir);

        let r = list_entries(&meta, &EntryFilter { limit: 10, ..Default::default() }).unwrap();
        assert_eq!(r.total, 3);
        assert_eq!(r.items.len(), 3);
        // level '' → 未分级；II → TOPIK II
        let ungraded = r.items.iter().find(|i| i.level_code.is_empty()).unwrap();
        assert_eq!(ungraded.level_label, "未分级");
        let lv2 = r.items.iter().find(|i| i.level_code == "II").unwrap();
        assert_eq!(lv2.level_label, "TOPIK II");

        // level 过滤
        let r2 = list_entries(
            &meta,
            &EntryFilter { level: Some("I"), limit: 10, ..Default::default() },
        )
        .unwrap();
        assert_eq!(r2.total, 1);
        assert_eq!(r2.items[0].headword, "-고");

        // category + q 过滤
        let r3 = list_entries(
            &meta,
            &EntryFilter {
                category: Some("助词"),
                q: Some("마저"),
                limit: 10,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(r3.total, 1);
        assert_eq!(r3.items[0].headword, "N마저");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn get_entry_returns_content_and_filters_hidden_images() {
        let dir = temp_dir("detail");
        seed_db(&dir);
        let meta = make_meta(&dir);
        let uid_ = uid::derive("kr-grammar", "-고");

        let mut hidden = HashSet::new();
        hidden.insert("a.jpeg".to_string()); // 用户已隐藏

        let payload = get_entry(&meta, &uid_, &hidden).unwrap();
        assert_eq!(payload.uid, uid_);
        assert_eq!(payload.headword, "-고");
        assert_eq!(payload.level_code, "I");
        assert_eq!(payload.level_label, "TOPIK I");
        assert_eq!(payload.tags, vec!["连接语尾", "接续"]);
        // a.jpeg 被 hidden 过滤；missing.jpeg 不存在被跳过 → 0 张
        assert_eq!(payload.images.len(), 0);

        // 不隐藏时：a.jpeg 存在返回，missing 跳过
        let payload2 = get_entry(&meta, &uid_, &HashSet::new()).unwrap();
        assert_eq!(payload2.images.len(), 1);
        assert_eq!(payload2.images[0].rel, "a.jpeg");
        assert!(payload2.images[0].path.ends_with("a.jpeg"));

        // 未知 uid → ENTRY_NOT_FOUND
        let err = get_entry(&meta, "kr-grammar:ffffffffffff", &HashSet::new()).unwrap_err();
        assert_eq!(err.code, "ENTRY_NOT_FOUND");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn unsafe_rel_skipped() {
        let dir = temp_dir("unsafe");
        seed_db(&dir);
        let meta = make_meta(&dir);
        let hidden = HashSet::new();
        let rels = vec![
            "a.jpeg".to_string(),
            "/etc/passwd".to_string(),
            "../secret".to_string(),
        ];
        let out = resolve_assets(meta.assets_dir.as_deref(), &rels, &hidden);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].rel, "a.jpeg");

        // 无 assets 目录 → 空
        let mut m2 = meta.clone();
        m2.assets_dir = None;
        let out2 = resolve_assets(m2.assets_dir.as_deref(), &rels, &hidden);
        assert_eq!(out2.len(), 0);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn helper_dto_refs() {
        let _ = LevelOpt { code: "I".into(), label: "TOPIK I".into(), rank: 1 };
        let _ = TypeOpt { value: "grammar".into(), label: "语法".into() };
    }
}
