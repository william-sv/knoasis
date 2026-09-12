// Knoasis · 用户数据持久化（userdata.db 可写库）
//
// 契约见 docs/Knoasis-学科数据组织与第三方接入方案.md §3.7 / §4.3。
//  - 位置：$APPDATA/com.william.knoasis/userdata.db（与 grammar/ 同级，启动时 mkdir -p + 建表）
//  - PRAGMA：WAL + busy_timeout=5000 + user_version=3
//  - user_version 2→3：krg: uid → kr-grammar: uid（favorite/note/image_hidden 重键）；新增 review 表
//  - 连接策略：managed state 持 Mutex<Connection>（可写单连接）；与 grammar 只读每命令一开互不冲突
//  - 不做外键级联：学科包替换/卸载时绝不删除用户数据（延续 v1.3「卸载保留」原则）
//  - 幂等 SQL：favorite INSERT ... ON CONFLICT(uid) DO NOTHING；note INSERT ... ON CONFLICT(uid) DO UPDATE

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State};

use crate::grammar::db::ApiError;
use crate::grammar::db::CmdResult;

/// Tauri managed state：可写 userdata 单连接。
pub struct UserData(pub Mutex<Connection>);

// DDL：favorite/note/image_hidden（v2 已存在）+ review（v3 新增）。
// 所有 CREATE TABLE IF NOT EXISTS —— 幂等，open_userdata 每次启动可重复执行。
const DDL: &str = r#"
CREATE TABLE IF NOT EXISTS favorite(
  uid         TEXT PRIMARY KEY,
  name        TEXT NOT NULL,
  type        TEXT,
  discipline  TEXT NOT NULL,
  category    TEXT,
  level_code  TEXT,
  summary     TEXT,
  created_at  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS ix_fav_created ON favorite(created_at DESC);

CREATE TABLE IF NOT EXISTS note(
  uid        TEXT PRIMARY KEY,
  content    TEXT NOT NULL DEFAULT '',
  updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS image_hidden(
  entry_uid TEXT NOT NULL,
  img_path  TEXT NOT NULL,
  hidden_at INTEGER NOT NULL,
  PRIMARY KEY (entry_uid, img_path)
);

CREATE TABLE IF NOT EXISTS review(
  uid           TEXT PRIMARY KEY,
  familiarity   INTEGER NOT NULL DEFAULT 0,
  review_count  INTEGER NOT NULL DEFAULT 0,
  last_reviewed INTEGER NOT NULL DEFAULT 0,
  next_review   INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS ix_review_next ON review(next_review);
"#;

// ---------------------------------------------------------------------------
// DTO
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FavoriteInput {
    pub uid: String,
    pub name: String,
    #[serde(rename = "type")]
    pub r#type: String,
    pub discipline: String,
    pub category: String,
    #[serde(rename = "level_code")]
    pub level_code: String,
    pub summary: String,
}

#[derive(Debug, Serialize)]
pub struct FavoriteRow {
    pub uid: String,
    pub name: String,
    #[serde(rename = "type")]
    pub r#type: String,
    pub discipline: String,
    pub category: String,
    #[serde(rename = "level_code")]
    pub level_code: String,
    pub summary: String,
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NoteInput {
    pub uid: String,
    pub content: String,
    #[serde(rename = "updated_at")]
    pub updated_at: i64,
}

#[derive(Debug, Serialize)]
pub struct NoteRow {
    pub uid: String,
    pub content: String,
    #[serde(rename = "updated_at")]
    pub updated_at: i64,
}

#[derive(Debug, Serialize)]
pub struct MigrateResult {
    #[serde(rename = "imported_favorites")]
    pub imported_favorites: usize,
    #[serde(rename = "imported_notes")]
    pub imported_notes: usize,
}

#[derive(Debug, Deserialize)]
pub struct MigrateLegacyArgs {
    pub favorites: Vec<FavoriteInput>,
    pub notes: Vec<NoteInput>,
}

#[derive(Debug, Deserialize)]
pub struct UidArgs {
    pub uid: String,
}

/// user_image_hide / user_image_restore 入参。
/// entry_uid = 条目 uid（kr-grammar:…，同 favorite 的 anchor 规则）；
/// img_path  = DB 内相对路径（包内相对，与 content JSON images[].rel 一致）。
#[derive(Debug, Deserialize)]
pub struct ImageHideArgs {
    #[serde(rename = "entry_uid")]
    pub entry_uid: String,
    #[serde(rename = "img_path")]
    pub img_path: String,
}

/// user_review_set 入参。familiarity 可选（0-5 熟悉度；缺省保持现有值）。
#[derive(Debug, Deserialize)]
pub struct ReviewSetArgs {
    pub uid: String,
    pub familiarity: Option<u8>,
}

/// user_review_get 返回
#[derive(Debug, Serialize)]
pub struct ReviewRow {
    pub uid: String,
    pub familiarity: i64,
    pub review_count: i64,
    #[serde(rename = "last_reviewed")]
    pub last_reviewed: i64,
    #[serde(rename = "next_review")]
    pub next_review: i64,
}

/// 查询某 entry_uid 已隐藏的图片相对路径集合（knowledge_get_entry 读取后过滤）。
pub fn image_hidden_set(ud: &UserData, entry_uid: &str) -> CmdResult<HashSet<String>> {
    let conn = ud
        .0
        .lock()
        .map_err(|_| ApiError::user_db_unavailable("userdata 锁被占用"))?;
    let mut stmt = conn
        .prepare("SELECT img_path FROM image_hidden WHERE entry_uid=?1")
        .map_err(|e| ApiError::user_db_unavailable(e.to_string()))?;
    let rows = stmt
        .query_map(params![entry_uid], |r| r.get::<_, String>(0))
        .map_err(|e| ApiError::user_db_unavailable(e.to_string()))?;
    let mut out = HashSet::new();
    for row in rows {
        out.insert(row.map_err(|e| ApiError::user_db_unavailable(e.to_string()))?);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// 打开 / 建表 / 迁移
// ---------------------------------------------------------------------------

fn read_user_version(conn: &Connection) -> CmdResult<i64> {
    conn.query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(|e| ApiError::user_db_unavailable(format!("读 user_version 失败：{e}")))
}

/// 迁移 v2 → v3：krg: uid → kr-grammar: uid + review 表（幂等；先清冲突后重键）。
/// 运行前提：DDL 已执行（favorite/note/image_hidden/review 表存在）。
fn migrate_v2_to_v3(conn: &Connection) -> CmdResult<()> {
    // 1. favorite 重键（uid 含学科前缀 + discipline 一并刷新）
    conn.execute(
        "DELETE FROM favorite WHERE uid LIKE 'kr-grammar:%' \
         AND substr(uid, 12) IN (SELECT substr(uid, 5) FROM favorite WHERE uid LIKE 'krg:%')",
        [],
    )
    .map_err(|e| ApiError::user_db_unavailable(format!("迁移 favorite 冲突清理失败：{e}")))?;
    conn.execute(
        "UPDATE favorite SET uid = 'kr-grammar:' || substr(uid, 5), discipline = 'kr-grammar' \
         WHERE uid LIKE 'krg:%'",
        [],
    )
    .map_err(|e| ApiError::user_db_unavailable(format!("迁移 favorite 重键失败：{e}")))?;

    // 2. note 重键
    conn.execute(
        "DELETE FROM note WHERE uid LIKE 'kr-grammar:%' \
         AND substr(uid, 12) IN (SELECT substr(uid, 5) FROM note WHERE uid LIKE 'krg:%')",
        [],
    )
    .map_err(|e| ApiError::user_db_unavailable(format!("迁移 note 冲突清理失败：{e}")))?;
    conn.execute(
        "UPDATE note SET uid = 'kr-grammar:' || substr(uid, 5) WHERE uid LIKE 'krg:%'",
        [],
    )
    .map_err(|e| ApiError::user_db_unavailable(format!("迁移 note 重键失败：{e}")))?;

    // 3. image_hidden 重键（复合主键按 entry_uid+img_path 清理冲突）
    conn.execute(
        "DELETE FROM image_hidden WHERE entry_uid LIKE 'kr-grammar:%' \
         AND (substr(entry_uid, 12), img_path) IN \
             (SELECT substr(entry_uid, 5), img_path FROM image_hidden WHERE entry_uid LIKE 'krg:%')",
        [],
    )
    .map_err(|e| ApiError::user_db_unavailable(format!("迁移 image_hidden 冲突清理失败：{e}")))?;
    conn.execute(
        "UPDATE image_hidden SET entry_uid = 'kr-grammar:' || substr(entry_uid, 5) \
         WHERE entry_uid LIKE 'krg:%'",
        [],
    )
    .map_err(|e| ApiError::user_db_unavailable(format!("迁移 image_hidden 重键失败：{e}")))?;

    Ok(())
}

pub fn open_userdata(app: &AppHandle) -> CmdResult<Connection> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| ApiError::user_db_unavailable(format!("无法定位应用数据目录：{e}")))?;
    std::fs::create_dir_all(&dir)
        .map_err(|e| ApiError::user_db_unavailable(format!("无法创建应用数据目录：{e}")))?;
    let path = dir.join("userdata.db");
    let conn = Connection::open(&path)
        .map_err(|e| ApiError::user_db_unavailable(format!("无法打开 userdata.db：{e}")))?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL; \
         PRAGMA busy_timeout=5000; \
         PRAGMA foreign_keys=ON;",
    )
    .map_err(|e| ApiError::user_db_unavailable(format!("初始化 userdata.db PRAGMA 失败：{e}")))?;

    // 建表（幂等）→ 迁移（如 <3）→ 设 user_version=3
    conn.execute_batch(DDL)
        .map_err(|e| ApiError::user_db_unavailable(format!("建表失败：{e}")))?;
    let version = read_user_version(&conn)?;
    if version < 3 {
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| ApiError::user_db_unavailable(format!("开启迁移事务失败：{e}")))?;
        migrate_v2_to_v3(&tx)?;
        tx.commit()
            .map_err(|e| ApiError::user_db_unavailable(format!("提交迁移事务失败：{e}")))?;
        conn.execute_batch("PRAGMA user_version=3;")
            .map_err(|e| ApiError::user_db_unavailable(format!("设置 user_version=3 失败：{e}")))?;
    }
    Ok(conn)
}

// ---------------------------------------------------------------------------
// 命令
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn user_favorites_list(state: State<'_, UserData>) -> CmdResult<Vec<FavoriteRow>> {
    let conn = state
        .0
        .lock()
        .map_err(|_| ApiError::user_db_unavailable("userdata 锁被占用"))?;
    let mut stmt = conn
        .prepare("SELECT uid, name, COALESCE(type,''), discipline, COALESCE(category,''), COALESCE(level_code,''), COALESCE(summary,''), created_at FROM favorite ORDER BY created_at DESC")
        .map_err(|e| ApiError::user_db_unavailable(e.to_string()))?;
    let rows = stmt
        .query_map([], |r| {
            Ok(FavoriteRow {
                uid: r.get(0)?,
                name: r.get(1)?,
                r#type: r.get(2)?,
                discipline: r.get(3)?,
                category: r.get(4)?,
                level_code: r.get(5)?,
                summary: r.get(6)?,
                created_at: r.get(7)?,
            })
        })
        .map_err(|e| ApiError::user_db_unavailable(e.to_string()))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| ApiError::user_db_unavailable(e.to_string()))?);
    }
    Ok(out)
}

#[tauri::command]
pub fn user_favorites_add(state: State<'_, UserData>, input: FavoriteInput) -> CmdResult<()> {
    let conn = state
        .0
        .lock()
        .map_err(|_| ApiError::user_db_unavailable("userdata 锁被占用"))?;
    let created_at = now_ms();
    conn.execute(
        "INSERT INTO favorite(uid, name, type, discipline, category, level_code, summary, created_at) \
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8) \
         ON CONFLICT(uid) DO NOTHING",
        params![
            input.uid,
            input.name,
            input.r#type,
            input.discipline,
            input.category,
            input.level_code,
            input.summary,
            created_at
        ],
    )
    .map_err(|e| ApiError::user_db_unavailable(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub fn user_favorites_remove(state: State<'_, UserData>, args: UidArgs) -> CmdResult<()> {
    let conn = state
        .0
        .lock()
        .map_err(|_| ApiError::user_db_unavailable("userdata 锁被占用"))?;
    conn.execute("DELETE FROM favorite WHERE uid=?1", params![args.uid])
        .map_err(|e| ApiError::user_db_unavailable(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub fn user_notes_list(state: State<'_, UserData>) -> CmdResult<Vec<NoteRow>> {
    let conn = state
        .0
        .lock()
        .map_err(|_| ApiError::user_db_unavailable("userdata 锁被占用"))?;
    let mut stmt = conn
        .prepare("SELECT uid, content, updated_at FROM note ORDER BY updated_at DESC")
        .map_err(|e| ApiError::user_db_unavailable(e.to_string()))?;
    let rows = stmt
        .query_map([], |r| {
            Ok(NoteRow { uid: r.get(0)?, content: r.get(1)?, updated_at: r.get(2)? })
        })
        .map_err(|e| ApiError::user_db_unavailable(e.to_string()))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| ApiError::user_db_unavailable(e.to_string()))?);
    }
    Ok(out)
}

#[tauri::command]
pub fn user_notes_save(state: State<'_, UserData>, input: NoteInput) -> CmdResult<()> {
    let conn = state
        .0
        .lock()
        .map_err(|_| ApiError::user_db_unavailable("userdata 锁被占用"))?;
    conn.execute(
        "INSERT INTO note(uid, content, updated_at) VALUES (?1,?2,?3) \
         ON CONFLICT(uid) DO UPDATE SET content=excluded.content, updated_at=excluded.updated_at",
        params![input.uid, input.content, input.updated_at],
    )
    .map_err(|e| ApiError::user_db_unavailable(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub fn user_migrate_legacy(
    state: State<'_, UserData>,
    args: MigrateLegacyArgs,
) -> CmdResult<MigrateResult> {
    let mut conn = state
        .0
        .lock()
        .map_err(|_| ApiError::user_db_unavailable("userdata 锁被占用"))?;
    let tx = conn
        .transaction()
        .map_err(|e| ApiError::user_db_unavailable(e.to_string()))?;
    let now = now_ms();
    for fav in args.favorites.iter() {
        tx.execute(
            "INSERT INTO favorite(uid, name, type, discipline, category, level_code, summary, created_at) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8) \
             ON CONFLICT(uid) DO NOTHING",
            params![
                fav.uid,
                fav.name,
                fav.r#type,
                fav.discipline,
                fav.category,
                fav.level_code,
                fav.summary,
                now
            ],
        )
        .map_err(|e| ApiError::user_db_unavailable(e.to_string()))?;
    }
    let imported_favorites = args.favorites.len();
    for note in args.notes.iter() {
        tx.execute(
            "INSERT INTO note(uid, content, updated_at) VALUES (?1,?2,?3) \
             ON CONFLICT(uid) DO UPDATE SET content=excluded.content, updated_at=excluded.updated_at",
            params![note.uid, note.content, note.updated_at],
        )
        .map_err(|e| ApiError::user_db_unavailable(e.to_string()))?;
    }
    let imported_notes = args.notes.len();
    tx.commit()
        .map_err(|e| ApiError::user_db_unavailable(e.to_string()))?;
    Ok(MigrateResult { imported_favorites, imported_notes })
}

#[tauri::command]
pub fn user_image_hide(state: State<'_, UserData>, args: ImageHideArgs) -> CmdResult<()> {
    let conn = state
        .0
        .lock()
        .map_err(|_| ApiError::user_db_unavailable("userdata 锁被占用"))?;
    let hidden_at = now_ms();
    conn.execute(
        "INSERT INTO image_hidden(entry_uid, img_path, hidden_at) VALUES (?1,?2,?3) \
         ON CONFLICT(entry_uid, img_path) DO NOTHING",
        params![args.entry_uid, args.img_path, hidden_at],
    )
    .map_err(|e| ApiError::user_db_unavailable(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub fn user_image_restore(state: State<'_, UserData>, args: ImageHideArgs) -> CmdResult<()> {
    let conn = state
        .0
        .lock()
        .map_err(|_| ApiError::user_db_unavailable("userdata 锁被占用"))?;
    conn.execute(
        "DELETE FROM image_hidden WHERE entry_uid=?1 AND img_path=?2",
        params![args.entry_uid, args.img_path],
    )
    .map_err(|e| ApiError::user_db_unavailable(e.to_string()))?;
    Ok(())
}

/// 记录一次复习事件：familiarity 可选（缺省保留现有）；review_count+1；last/next=now。
#[tauri::command]
pub fn user_review_set(state: State<'_, UserData>, args: ReviewSetArgs) -> CmdResult<()> {
    let conn = state
        .0
        .lock()
        .map_err(|_| ApiError::user_db_unavailable("userdata 锁被占用"))?;
    let now = now_ms();
    let familiarity_i64 = args.familiarity.map(|f| i64::from(f));
    // 已有行取当前 familiarity（familiarity 未提供时保留）
    let current: Option<i64> = conn
        .query_row(
            "SELECT familiarity FROM review WHERE uid=?1",
            params![args.uid],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| ApiError::user_db_unavailable(e.to_string()))?;
    let familiarity = familiarity_i64.or(current).unwrap_or(0).clamp(0, 5);
    conn.execute(
        "INSERT INTO review(uid, familiarity, review_count, last_reviewed, next_review) \
         VALUES (?1,?2,1,?3,?4) \
         ON CONFLICT(uid) DO UPDATE SET \
           familiarity=excluded.familiarity, \
           review_count=review_count+1, \
           last_reviewed=excluded.last_reviewed, \
           next_review=excluded.next_review",
        params![args.uid, familiarity, now, now],
    )
    .map_err(|e| ApiError::user_db_unavailable(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub fn user_review_get(state: State<'_, UserData>, args: UidArgs) -> CmdResult<Option<ReviewRow>> {
    let conn = state
        .0
        .lock()
        .map_err(|_| ApiError::user_db_unavailable("userdata 锁被占用"))?;
    let row = conn
        .query_row(
            "SELECT uid, familiarity, review_count, last_reviewed, next_review \
             FROM review WHERE uid=?1",
            params![args.uid],
            |r| {
                Ok(ReviewRow {
                    uid: r.get(0)?,
                    familiarity: r.get(1)?,
                    review_count: r.get(2)?,
                    last_reviewed: r.get(3)?,
                    next_review: r.get(4)?,
                })
            },
        )
        .optional()
        .map_err(|e| ApiError::user_db_unavailable(e.to_string()))?;
    Ok(row)
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// 单元测试（纯 SQL 语义，不依赖 tauri 运行环境）
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn mem() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(DDL).unwrap();
        conn.execute_batch("PRAGMA user_version=2;").unwrap();
        conn
    }

    #[test]
    fn image_hidden_ddl_idempotent_hide_dup_restore_idempotent() {
        let conn = mem();
        // DDL 幂等：连续建两次不报错
        conn.execute_batch(DDL).unwrap();

        // hide：同 (entry_uid,img_path) 重复执行不产生重复行（ON CONFLICT DO NOTHING）
        for _ in 0..2 {
            conn.execute(
                "INSERT INTO image_hidden(entry_uid, img_path, hidden_at) VALUES (?1,?2,?3) \
                 ON CONFLICT(entry_uid, img_path) DO NOTHING",
                params!["en:abc", "book/x.jpeg", 1i64],
            )
            .unwrap();
        }
        // 另一条目/另一图可并存
        conn.execute(
            "INSERT INTO image_hidden(entry_uid, img_path, hidden_at) VALUES (?1,?2,?3) \
             ON CONFLICT(entry_uid, img_path) DO NOTHING",
            params!["en:abc", "book/y.jpeg", 1i64],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO image_hidden(entry_uid, img_path, hidden_at) VALUES (?1,?2,?3) \
             ON CONFLICT(entry_uid, img_path) DO NOTHING",
            params!["en:other", "book/x.jpeg", 1i64],
        )
        .unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM image_hidden", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 3);

        // restore：DELETE 幂等（再删不报错，行数 0）
        for _ in 0..2 {
            conn.execute(
                "DELETE FROM image_hidden WHERE entry_uid=?1 AND img_path=?2",
                params!["en:abc", "book/x.jpeg"],
            )
            .unwrap();
        }
        let remain: i64 = conn
            .query_row("SELECT COUNT(*) FROM image_hidden WHERE entry_uid='en:abc'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(remain, 1); // 只剩 book/y.jpeg
    }

    #[test]
    fn migrate_v2_to_v3_rekeys_krg_and_keeps_en() {
        let conn = mem();
        // krg: 旧格式
        conn.execute(
            "INSERT INTO favorite(uid,name,type,discipline,category,level_code,summary,created_at) \
             VALUES ('krg:1f2e3d4c5b6a','N마저','grammar','kr-grammar','助词','II','s',1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO favorite(uid,name,type,discipline,category,level_code,summary,created_at) \
             VALUES ('krg:1f2e3d4c5b6a-2','重复','grammar','kr-grammar','助词','II','s',1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO favorite(uid,name,type,discipline,category,level_code,summary,created_at) \
             VALUES ('en:aaaaaaaaaaaa','English','grammar','en-grammar','句法','III','s',1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO note(uid,content,updated_at) VALUES ('krg:1f2e3d4c5b6a','n',1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO image_hidden(entry_uid,img_path,hidden_at) VALUES ('krg:1f2e3d4c5b6a','x',1)",
            [],
        )
        .unwrap();

        migrate_v2_to_v3(&conn).unwrap();

        let favs: Vec<(String, String)> = {
            let mut stmt = conn
                .prepare("SELECT uid, discipline FROM favorite ORDER BY uid")
                .unwrap();
            let rows = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap();
            rows.collect::<Result<_, _>>().unwrap()
        };
        assert!(favs.iter().all(|(uid, _)| !uid.starts_with("krg:")));
        assert!(favs.iter().any(|(uid, d)| uid == "kr-grammar:1f2e3d4c5b6a" && d == "kr-grammar"));
        assert!(favs.iter().any(|(uid, _)| uid == "kr-grammar:1f2e3d4c5b6a-2"));
        assert!(favs.iter().any(|(uid, _)| uid == "en:aaaaaaaaaaaa"));

        let notes: Vec<String> = {
            let mut stmt = conn.prepare("SELECT uid FROM note").unwrap();
            let rows = stmt.query_map([], |r| r.get(0)).unwrap();
            rows.collect::<Result<_, _>>().unwrap()
        };
        assert!(notes.iter().all(|u| !u.starts_with("krg:")));
        assert_eq!(notes, vec!["kr-grammar:1f2e3d4c5b6a".to_string()]);

        let ih: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT entry_uid FROM image_hidden")
                .unwrap();
            let rows = stmt.query_map([], |r| r.get(0)).unwrap();
            rows.collect::<Result<_, _>>().unwrap()
        };
        assert_eq!(ih, vec!["kr-grammar:1f2e3d4c5b6a".to_string()]);
    }

    #[test]
    fn review_upsert_and_read() {
        let conn = mem();
        // set（无 familiarity → 新行 0；count 1）
        let now = 1000;
        conn.execute(
            "INSERT INTO review(uid, familiarity, review_count, last_reviewed, next_review) \
             VALUES ('kr-grammar:x', 0, 1, ?1, ?1) \
             ON CONFLICT(uid) DO UPDATE SET familiarity=excluded.familiarity, review_count=review_count+1, \
               last_reviewed=excluded.last_reviewed, next_review=excluded.next_review",
            params![now],
        )
        .unwrap();
        let (fam, cnt): (i64, i64) = conn
            .query_row(
                "SELECT familiarity, review_count FROM review WHERE uid='kr-grammar:x'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((fam, cnt), (0, 1));

        // 再次 upsert：review_count+1
        conn.execute(
            "INSERT INTO review(uid, familiarity, review_count, last_reviewed, next_review) \
             VALUES ('kr-grammar:x', 3, 1, ?1, ?1) \
             ON CONFLICT(uid) DO UPDATE SET familiarity=excluded.familiarity, review_count=review_count+1, \
               last_reviewed=excluded.last_reviewed, next_review=excluded.next_review",
            params![now],
        )
        .unwrap();
        let (fam2, cnt2): (i64, i64) = conn
            .query_row(
                "SELECT familiarity, review_count FROM review WHERE uid='kr-grammar:x'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((fam2, cnt2), (3, 2));
    }
}
