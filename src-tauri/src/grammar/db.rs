// Knoasis · grammar.db 只读访问层（多学科：kr_grammar / en_grammar）
//
// 职责（对齐 docs/Knoasis-英语语法接入设计.md §2/§3/§4，并向下兼容韩语接入设计 §1.2/§2）：
//  1. DB 路径三优先级解析：$KNASIS_GRAMMAR_DB → $APPDATA/.../grammar/grammar.db → <resource_dir>/grammar/grammar.db
//  2. 每命令新建只读连接（SQLITE_OPEN_READ_ONLY | SQLITE_OPEN_NO_MUTEX），不持有长锁
//  3. DTO 与 IPC 返回 JSON 形状（模板、Rust DTO、前端 adapter 三方字段键对齐）
//  4. uid 派生按学科前缀：krg = "krg:"+hex(sha1(headword))[0..12]（冻结）；en = "en:"+hex(sha1(headword))[0..12]
//  5. 学科布局（SubjectProfile）：detail 表名 / summary 表达式 / 可搜索列等从常量 match（无注入面）
//
// 本文件只依赖 rusqlite / serde / serde_json / tauri 路径 API，不含前端逻辑。

use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde::Serialize;
use sha1::{Digest, Sha1};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use tauri::Manager;

/// 韩语语法学科的强调色（冻结，与 mock 的 english 蓝 / math 紫区分）
pub const GRAMMAR_COLOR: &str = "#D97706";
/// 韩语语法学科 code（subjects.code / entries.subject_code）
pub const GRAMMAR_SUBJECT: &str = "kr_grammar";
/// 前端 UI 侧的 set.id / discipline（adapter 直接使用）
pub const GRAMMAR_DISCIPLINE: &str = "kr-grammar";

/// 英语语法学科 code（subjects.code / entries.subject_code）
pub const EN_GRAMMAR_SUBJECT: &str = "en_grammar";
/// 英语语法前端 UI 侧 set.id / discipline
pub const EN_GRAMMAR_DISCIPLINE: &str = "en-grammar";
/// 英语语法学科的强调色
pub const EN_GRAMMAR_COLOR: &str = "#2563EB";

// ---------------------------------------------------------------------------
// 错误类型：统一 { code, message }，前端 toast 显示 message
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct ApiError {
    pub code: &'static str,
    pub message: String,
}

impl ApiError {
    pub fn db_unavailable(msg: impl Into<String>) -> Self {
        ApiError { code: "DB_UNAVAILABLE", message: msg.into() }
    }
    pub fn schema_mismatch(msg: impl Into<String>) -> Self {
        ApiError { code: "SCHEMA_MISMATCH", message: msg.into() }
    }
    pub fn entry_not_found(uid: impl Into<String>) -> Self {
        ApiError { code: "ENTRY_NOT_FOUND", message: uid.into() }
    }
    /// knowledge 学科包不存在（knowledge_list_entries 传了未知 set_id 等）
    pub fn set_not_found(msg: impl Into<String>) -> Self {
        ApiError { code: "SET_NOT_FOUND", message: msg.into() }
    }
    pub fn user_db_unavailable(msg: impl Into<String>) -> Self {
        ApiError { code: "USER_DB_UNAVAILABLE", message: msg.into() }
    }
    pub fn internal(msg: impl Into<String>) -> Self {
        ApiError { code: "INTERNAL", message: msg.into() }
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for ApiError {}

pub type CmdResult<T> = Result<T, ApiError>;

// ---------------------------------------------------------------------------
// 学科布局（SubjectProfile）—— 常量 match，无注入面
// ---------------------------------------------------------------------------

/// 静态学科布局。`summary_expr` 为 SELECT 列表达式（引用别名 d=detail 表），
/// `search_extra_col` 为搜索额外命中列（引用 d；'' 表示无额外列，kr 保持原行为）。
pub struct SubjectProfile {
    pub code: &'static str,
    pub discipline: &'static str,
    pub color: &'static str,
    pub level_system: &'static str,
    pub level_system_label: &'static str,
    pub detail_table: &'static str,
    pub summary_expr: &'static str,
    pub search_extra_col: &'static str,
    pub description: &'static str,
}

const KR_PROFILE: SubjectProfile = SubjectProfile {
    code: GRAMMAR_SUBJECT,
    discipline: GRAMMAR_DISCIPLINE,
    color: GRAMMAR_COLOR,
    level_system: "topik",
    level_system_label: "TOPIK",
    detail_table: "kr_grammar_detail",
    summary_expr: "COALESCE(d.meaning_brief,'')",
    search_extra_col: "",
    description: "韩语语法 · TOPIK I/II 分级 · 642 条真实语法点",
};

const EN_PROFILE: SubjectProfile = SubjectProfile {
    code: EN_GRAMMAR_SUBJECT,
    discipline: EN_GRAMMAR_DISCIPLINE,
    color: EN_GRAMMAR_COLOR,
    level_system: "stage",
    level_system_label: "难度",
    detail_table: "en_grammar_detail",
    summary_expr: "COALESCE(d.summary,'')",
    search_extra_col: "COALESCE(d.explanation,'')",
    description: "英语语法 · 21 类 / 382 条语法点 · 740 张图解",
};

/// 取学科布局；未知学科返回 None（list_sets 对未知学科走既有通用分支）。
fn subject_profile(code: &str) -> Option<&'static SubjectProfile> {
    let out = match code {
        GRAMMAR_SUBJECT => Some(&KR_PROFILE),
        EN_GRAMMAR_SUBJECT => Some(&EN_PROFILE),
        _ => None,
    };
    debug_assert!(out.map_or(true, |p| p.code == code));
    out
}

/// set_id → subject_code。缺省 / 未知 → kr（向后兼容：今日前端不带 set_id 仍取韩语）。
pub fn subject_code_of(set_id: Option<&str>) -> &'static str {
    match set_id {
        Some(s) => match s.trim() {
            "en-grammar" | "en_grammar" => EN_GRAMMAR_SUBJECT,
            _ => GRAMMAR_SUBJECT,
        },
        None => GRAMMAR_SUBJECT,
    }
}

/// uid 前缀 → subject_code。`en:` → en；其余（`krg:` / 未知）→ kr（向后兼容）。
/// uid 由学科前缀 + hex(sha1(headword))[0..12] 派生（list_entries 撞车时可能追加 -2 后缀），
/// 因此只按前缀判断，不校验长度。
pub fn subject_from_uid(uid: &str) -> &'static str {
    if let Some(_rest) = uid.strip_prefix("en:") {
        return EN_GRAMMAR_SUBJECT;
    }
    GRAMMAR_SUBJECT
}

// ---------------------------------------------------------------------------
// DTO（列表 / 学科元信息 / 详情）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct LevelOpt {
    pub code: String,
    pub label: String,
    pub rank: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct TypeOpt {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct SubjectCounts {
    pub total: usize,
    pub by_level: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubjectMeta {
    pub id: String,
    pub name: String,
    pub color: String,
    pub discipline: String,
    pub level_system: String,
    pub level_system_label: String,
    pub levels: Vec<LevelOpt>,
    pub types: Vec<TypeOpt>,
    pub template: serde_json::Value,
    pub counts: SubjectCounts,
    pub description: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EntryItem {
    pub uid: String,
    pub headword: String,
    pub category: String,
    pub level_code: String,
    pub level_label: String,
    pub tags: Vec<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ListEntriesResult {
    pub total: usize,
    pub items: Vec<EntryItem>,
}

// ---- 韩语详情 DTO（冻结：JSON 契约逐字节不变） ----

#[derive(Debug, Clone, Serialize)]
pub struct Mean {
    pub meaning_zh: String,
    pub meaning_ko: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Usage {
    pub title: String,
    pub usage_zh: String,
    pub usage_ko: String,
    pub form: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Scene {
    pub scene: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Example {
    pub ko: String,
    pub zh: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Caution {
    pub caution: String,
    pub wrong: String,
    pub right: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Collocation {
    pub word: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Related {
    pub uid: String,
    pub headword: String,
    pub relation: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RelatedPending {
    pub target_text: String,
    pub relation: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EntryDetail {
    pub uid: String,
    pub headword: String,
    pub category: String,
    pub level_code: String,
    pub level_label: String,
    pub tags: Vec<String>,
    pub pattern: String,
    pub variants: Vec<String>,
    pub form_rule: String,
    pub attaches_to: Vec<String>,
    pub speech_style: String,
    pub politeness: String,
    pub register: String,
    pub irregular: bool,
    pub difficulty: Option<u8>,
    pub frequency: String,
    pub meaning_brief: String,
    pub usage_scene: String,
    pub notes: String,
    pub meanings: Vec<Mean>,
    pub usages: Vec<Usage>,
    pub scenes: Vec<Scene>,
    pub examples: Vec<Example>,
    pub cautions: Vec<Caution>,
    pub collocations: Vec<Collocation>,
    pub related: Vec<Related>,
    pub related_pending: Vec<RelatedPending>,
}

// ---- 英语详情 DTO（对齐 en_grammar 模板 sections 键） ----

#[derive(Debug, Clone, Serialize)]
pub struct EnExample {
    pub en: String,
    pub zh: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EnImage {
    pub rel: String,  // DB 内相对路径（<book>/<file>），也是 image_hidden 主键值
    pub path: String, // 已解析本地绝对路径：前端 assetUrl(path) 渲染 <img>
}

#[derive(Debug, Clone, Serialize)]
pub struct EnEntryDetail {
    pub uid: String,
    pub headword: String,
    pub category: String,
    pub level_code: String,
    pub level_label: String,
    pub tags: Vec<String>,
    pub summary: String,
    pub explanation: String,
    pub examples: Vec<EnExample>,
    pub images: Vec<EnImage>,
}

/// grammar_get_detail 返回载荷：untagged 枚举 → kr 序列化与冻结 JSON 完全一致。
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum EntryDetailPayload {
    Kr(EntryDetail),
    En(EnEntryDetail),
}

#[derive(Debug, Clone, Serialize)]
pub struct DbInfo {
    pub path: String,
    pub entries: usize,
    pub db_updated_at: Option<String>,
}

// ---------------------------------------------------------------------------
// uid 派生与等级映射
// ---------------------------------------------------------------------------

/// 派生 uid 的 hex 主体：hex(sha1(headword))[0..12]
fn uid_hex(headword: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(headword.as_bytes());
    let digest = hasher.finalize();
    let hex: String = digest.iter().map(|b| format!("{:02x}", b)).collect();
    hex[..12].to_string()
}

/// uid = "krg:" + hex(sha1(headword))[0..12]；headword 变化才变化，重导入稳定。
pub fn uid_of(headword: &str) -> String {
    format!("krg:{}", uid_hex(headword))
}

/// 按学科前缀派生 uid；kr=krg:、en=en:。
pub fn uid_of_for(subject: &str, headword: &str) -> String {
    let prefix = if subject == EN_GRAMMAR_SUBJECT { "en:" } else { "krg:" };
    format!("{prefix}{}", uid_hex(headword))
}

/// level TEXT → code（NULL / 空白一律归一为 ''，表示「未分级」）
pub fn level_code(raw: Option<&str>) -> String {
    match raw {
        Some(s) if !s.trim().is_empty() => s.trim().to_string(),
        _ => String::new(),
    }
}

/// code → 展示用 label 与排序 rank（韩语：I→TOPIK I(1)、II→TOPIK II(2)、''→未分级(0)）
pub fn level_meta(code: &str) -> (String, u32) {
    match code {
        "I" => ("TOPIK I".to_string(), 1),
        "II" => ("TOPIK II".to_string(), 2),
        _ => ("未分级".to_string(), 0),
    }
}

/// 按学科返回 level 映射（kr 保持原逻辑；en：I→基础/II→进阶/III→高级）。
pub fn level_meta_for(subject: &str, code: &str) -> (String, u32) {
    if subject == EN_GRAMMAR_SUBJECT {
        return match code {
            "I" => ("基础".to_string(), 1),
            "II" => ("进阶".to_string(), 2),
            "III" => ("高级".to_string(), 3),
            _ => ("未分级".to_string(), 0),
        };
    }
    level_meta(code)
}

/// 解析 JSON 字符串数组列；解析失败回退按中文顿号/逗号切分，仍失败给空数组（容错不 panic）。
pub fn parse_string_array(json: Option<&str>) -> Vec<String> {
    let Some(raw) = json else { return Vec::new() };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    if let Ok(v) = serde_json::from_str::<Vec<String>>(trimmed) {
        return v.into_iter().filter(|s| !s.trim().is_empty()).collect();
    }
    // 部分旧数据可能是 "A、B" 而非 JSON 数组
    trimmed
        .split(['、', ',', '，', ';', '；'])
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

fn split_tags(tags_text: &str, joined_tags: Option<&str>) -> Vec<String> {
    let mut seen = HashMap::new();
    let mut out: Vec<String> = Vec::new();
    let push = |tag: &str, seen: &mut HashMap<String, ()>, out: &mut Vec<String>| {
        let t = tag.trim();
        if t.is_empty() || seen.contains_key(t) {
            return;
        }
        seen.insert(t.to_string(), ());
        out.push(t.to_string());
    };
    for part in tags_text.split(|c| matches!(c, ',' | '，' | '、' | ';' | '；' | '|')) {
        push(part, &mut seen, &mut out);
    }
    if let Some(joined) = joined_tags {
        for part in joined.split('|') {
            push(part, &mut seen, &mut out);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// 路径解析 / 连接 / schema 校验
// ---------------------------------------------------------------------------

/// 解析 grammar.db 三优先级路径；只返回确实存在的文件。
pub fn resolve_grammar_db(app: &tauri::AppHandle) -> CmdResult<PathBuf> {
    // 1. $KNASIS_GRAMMAR_DB（dev 便捷覆盖）
    if let Ok(p) = std::env::var("KNASIS_GRAMMAR_DB") {
        let pb = PathBuf::from(p);
        if pb.is_file() {
            return Ok(pb);
        }
    }
    // 2. $APPDATA/com.william.knoasis/grammar/grammar.db
    if let Ok(dir) = app.path().app_data_dir() {
        let pb = dir.join("grammar").join("grammar.db");
        if pb.is_file() {
            return Ok(pb);
        }
    }
    // 3. 随包资源 <resource_dir>/grammar/grammar.db（tauri build 打包后生效）
    if let Ok(dir) = app.path().resource_dir() {
        let pb = dir.join("grammar").join("grammar.db");
        if pb.is_file() {
            return Ok(pb);
        }
    }
    // 4. dev 便捷兜底：仓库资源 src-tauri/resources/grammar/grammar.db
    //    （bundle.resources 只在打包时拷贝，tauri dev 下 resource_dir 拿不到，此档保证开箱即用）
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("grammar")
        .join("grammar.db");
    if dev.is_file() {
        return Ok(dev);
    }
    Err(ApiError::db_unavailable(
        "未找到 grammar.db：请设置 KNASIS_GRAMMAR_DB 或运行 scripts/sync-grammar-db.sh",
    ))
}

/// 解析 en_images 目录（英语图解），与 grammar.db 同根：
/// 1. $KNASIS_GRAMMAR_IMAGES（显式覆盖，调试）
/// 2. <grammar.db 同目录>/en_images
/// 目录不存在返回 None（get_detail_en 里 images=[]，模板自动隐藏「图解」节，不报错）。
pub fn resolve_en_images_dir(app: &tauri::AppHandle) -> Option<PathBuf> {
    // 1. 显式覆盖（调试）
    if let Ok(dir) = std::env::var("KNASIS_GRAMMAR_IMAGES") {
        let pb = PathBuf::from(dir);
        if pb.is_dir() {
            return Some(pb);
        }
    }
    // 2. 与解析出的 grammar.db 同根：<db_dir>/en_images
    let db = resolve_grammar_db(app).ok()?;
    let dir = db.parent()?.join("en_images");
    if dir.is_dir() {
        Some(dir)
    } else {
        None
    }
}

/// 每命令新建只读连接：库毫秒级打开，避免跨命令长连接持锁。
pub fn open_grammar_ro(path: &Path) -> CmdResult<Connection> {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| ApiError::db_unavailable(format!("无法打开 grammar.db：{e}")))
}

/// schema 校验：基础三表必须存在；subjects 含 en_grammar 时，en 专属表缺失报 SCHEMA_MISMATCH。
fn ensure_schema(conn: &Connection) -> CmdResult<()> {
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name IN (?1,?2,?3)")
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let rows = stmt
        .query_map(params!["entries", "kr_grammar_detail", "subjects"], |r| {
            r.get::<_, String>(0)
        })
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut found = Vec::new();
    for row in rows {
        found.push(row.map_err(|e| ApiError::internal(e.to_string()))?);
    }
    for need in ["entries", "kr_grammar_detail", "subjects"] {
        if !found.iter().any(|f| f == need) {
            return Err(ApiError::schema_mismatch(format!(
                "grammar.db 缺表 {need}：数据文件版本不符，请升级数据文件"
            )));
        }
    }

    // en 学科注册存在 → 校验 en 专属表（英语语法接入设计 §3.5）
    let has_en: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM subjects WHERE code=?1",
            params![EN_GRAMMAR_SUBJECT],
            |r| r.get(0),
        )
        .map_err(|e| ApiError::internal(e.to_string()))?;
    if has_en > 0 {
        let mut en_stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name IN (?1,?2)")
            .map_err(|e| ApiError::internal(e.to_string()))?;
        let en_rows = en_stmt
            .query_map(params!["en_grammar_detail", "en_grammar_examples"], |r| {
                r.get::<_, String>(0)
            })
            .map_err(|e| ApiError::internal(e.to_string()))?;
        let mut en_found = Vec::new();
        for row in en_rows {
            en_found.push(row.map_err(|e| ApiError::internal(e.to_string()))?);
        }
        for need in ["en_grammar_detail", "en_grammar_examples"] {
            if !en_found.iter().any(|f| f == need) {
                return Err(ApiError::schema_mismatch(format!(
                    "grammar.db 已注册英语语法但缺表 {need}：数据文件版本不符，请升级数据文件"
                )));
            }
        }
    }
    Ok(())
}

fn db_updated_at(conn: &Connection) -> Option<String> {
    conn.query_row(
        "SELECT MAX(updated_at) FROM entries WHERE updated_at IS NOT NULL",
        [],
        |r| r.get(0),
    )
    .ok()
    .flatten()
}

// ---------------------------------------------------------------------------
// 学科列表（grammar_list_sets）
// ---------------------------------------------------------------------------

fn template_section(registry: &serde_json::Value, key: &str) -> serde_json::Value {
    registry
        .get(key)
        .cloned()
        .unwrap_or_else(|| serde_json::Value::Null)
}

pub fn list_sets(conn: &Connection, registry: &serde_json::Value) -> CmdResult<Vec<SubjectMeta>> {
    let mut stmt = conn
        .prepare("SELECT code, name, template FROM subjects ORDER BY id")
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
        })
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let mut metas = Vec::new();
    for row in rows {
        let (code, name, template_key) = row.map_err(|e| ApiError::internal(e.to_string()))?;
        let profile = subject_profile(&code);

        // 各等级计数（含 '' 未分级），用于 counts.by_level
        let mut counts = SubjectCounts::default();
        {
            let mut cstmt = conn
                .prepare("SELECT COALESCE(level,''), COUNT(*) FROM entries WHERE subject_code=?1 GROUP BY COALESCE(level,'')")
                .map_err(|e| ApiError::internal(e.to_string()))?;
            let crows = cstmt
                .query_map(params![code], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, usize>(1)?))
                })
                .map_err(|e| ApiError::internal(e.to_string()))?;
            for c in crows {
                let (lv, n) = c.map_err(|e| ApiError::internal(e.to_string()))?;
                counts.total += n;
                counts.by_level.insert(lv, n);
            }
        }

        // 非空 level 去重后排序（'' 未分级不进入 set.levels，保持现有筛选下拉不变）
        let mut level_codes: Vec<String> = counts
            .by_level
            .keys()
            .filter(|c| !c.is_empty())
            .cloned()
            .collect();
        level_codes.sort_by_key(|c| level_meta_for(&code, c).1);
        let levels: Vec<LevelOpt> = level_codes
            .into_iter()
            .map(|c| {
                let (label, rank) = level_meta_for(&code, &c);
                LevelOpt { code: c, label, rank }
            })
            .collect();

        // 学科默认 type（语法单类型）
        let types = vec![TypeOpt { value: "grammar".to_string(), label: "语法".to_string() }];

        metas.push(SubjectMeta {
            id: code.clone(),
            name: name.clone(),
            color: profile
                .map(|p| p.color.to_string())
                .unwrap_or_else(|| "#4A90D9".to_string()),
            discipline: profile
                .map(|p| p.discipline.to_string())
                .unwrap_or_else(|| code.clone()),
            level_system: profile
                .map(|p| p.level_system.to_string())
                .unwrap_or_else(|| "custom".to_string()),
            level_system_label: profile
                .map(|p| p.level_system_label.to_string())
                .unwrap_or_else(|| "等级".to_string()),
            levels,
            types,
            template: template_section(registry, &template_key),
            counts,
            description: profile
                .map(|p| p.description.to_string())
                .unwrap_or_else(|| format!("{name}")),
        });
    }
    Ok(metas)
}

// ---------------------------------------------------------------------------
// 条目列表（grammar_list_entries）
// ---------------------------------------------------------------------------

pub fn list_entries(
    conn: &Connection,
    subject_code: &str,
    _set_id: Option<&str>,
    level: Option<&str>,
    category: Option<&str>,
    q: Option<&str>,
    limit: u32,
) -> CmdResult<ListEntriesResult> {
    // 学科布局（未知学科回退 kr 布局，保证既有调用路径不变）
    let profile = subject_profile(subject_code).unwrap_or(&KR_PROFILE);
    let detail_table = profile.detail_table;
    let summary_expr = profile.summary_expr;
    let search_extra_col = profile.search_extra_col;
    let subject_for_level = subject_code;

    let level_cond = level.filter(|s| !s.is_empty());
    let cat_cond = category.filter(|s| !s.is_empty());
    let q_cond = q.map(|s| s.trim()).filter(|s| !s.is_empty());

    let mut sql = String::from(
        "SELECT e.id, e.headword, e.category, COALESCE(e.level,''), COALESCE(e.tags_text,''), ",
    );
    sql.push_str(summary_expr);
    sql.push_str(", GROUP_CONCAT(t.tag, '|') \
         FROM entries e \
         LEFT JOIN ");
    sql.push_str(detail_table);
    sql.push_str(" d ON d.entry_id = e.id \
         LEFT JOIN entry_tags t ON t.entry_id = e.id \
         WHERE e.subject_code = ? ");
    let mut pv: Vec<rusqlite::types::Value> = vec![subject_code.to_string().into()];
    if let Some(lv) = level_cond {
        sql.push_str(" AND COALESCE(e.level,'') = ?");
        pv.push(lv.to_string().into());
    }
    if let Some(cat) = cat_cond {
        sql.push_str(" AND COALESCE(e.category,'') = ?");
        pv.push(cat.to_string().into());
    }
    if let Some(query) = q_cond {
        sql.push_str(" AND (e.headword LIKE ? OR ");
        sql.push_str(summary_expr);
        sql.push_str(" LIKE ? OR COALESCE(e.tags_text,'') LIKE ?");
        if !search_extra_col.is_empty() {
            sql.push_str(" OR ");
            sql.push_str(search_extra_col);
            sql.push_str(" LIKE ?");
        }
        sql.push_str(")");
        let like = format!("%{}%", query);
        pv.push(like.clone().into());
        pv.push(like.clone().into());
        pv.push(like.clone().into());
        if !search_extra_col.is_empty() {
            pv.push(like.into());
        }
    }
    sql.push_str(" GROUP BY e.id ORDER BY e.category, e.headword LIMIT ?");
    pv.push((limit as i64).into());

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(pv.iter()), |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<String>>(6)?,
            ))
        })
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let mut items: Vec<EntryItem> = Vec::new();
    let mut seen_uid: HashMap<String, i64> = HashMap::new();
    for row in rows {
        let (id, headword, category, lv, tags_text, summary, joined_tags) =
            row.map_err(|e| ApiError::internal(e.to_string()))?;
        let code = level_code(Some(&lv));
        let (label, _rank) = level_meta_for(subject_for_level, &code);
        let mut uid = uid_of_for(subject_code, &headword);
        if seen_uid.contains_key(&uid) {
            let mut n = 2;
            while seen_uid.contains_key(&format!("{uid}-{n}")) {
                n += 1;
            }
            uid = format!("{uid}-{n}");
        }
        seen_uid.insert(uid.clone(), id);
        items.push(EntryItem {
            uid,
            headword,
            category: category.unwrap_or_default(),
            level_code: code,
            level_label: label,
            tags: split_tags(&tags_text, joined_tags.as_deref()),
            summary: summary.unwrap_or_default(),
        });
    }
    let total = items.len();
    Ok(ListEntriesResult { total, items })
}

// ---------------------------------------------------------------------------
// 详情（grammar_get_detail）：按学科分派 get_detail_kr / get_detail_en
// ---------------------------------------------------------------------------

struct EntryRef {
    id: i64,
    headword: String,
    category: String,
    level_code: String,
    tags: Vec<String>,
}

fn find_entry_by_uid(
    conn: &Connection,
    subject: &str,
    uid: &str,
) -> CmdResult<Option<EntryRef>> {
    let mut stmt = conn
        .prepare("SELECT id, headword, category, COALESCE(level,''), COALESCE(tags_text,'') FROM entries WHERE subject_code = ?1")
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let rows = stmt
        .query_map(params![subject], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
            ))
        })
        .map_err(|e| ApiError::internal(e.to_string()))?;
    for row in rows {
        let (id, headword, category, lv, tags_text) =
            row.map_err(|e| ApiError::internal(e.to_string()))?;
        // uid 由 headword 派生；sha1 撞车在 list_entries 中会加 "-2" 后缀，
        // 因此此处同时尝试基础 uid 与 "-2" 补偿 uid（实际数据集不会触发）。
        let base_uid = uid_of_for(subject, &headword);
        let candidates = [base_uid.clone(), format!("{base_uid}-2")];
        if candidates.iter().any(|c| c == uid) {
            let code = level_code(Some(&lv));
            return Ok(Some(EntryRef {
                id,
                headword,
                category: category.unwrap_or_default(),
                level_code: code,
                tags: split_tags(&tags_text, None),
            }));
        }
    }
    Ok(None)
}

fn row_mean(r: &rusqlite::Row) -> rusqlite::Result<Mean> {
    Ok(Mean {
        meaning_zh: r.get(0)?,
        meaning_ko: r.get(1)?,
        note: r.get(2)?,
    })
}

/// 韩语详情：1 + 7 段 SQL 拼装，逻辑与 JSON 契约保持原样（英语接入回归点）。
pub fn get_detail_kr(conn: &Connection, uid: &str) -> CmdResult<EntryDetail> {
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let entry = find_entry_by_uid(&tx, GRAMMAR_SUBJECT, uid)?
        .ok_or_else(|| ApiError::entry_not_found(uid.to_string()))?;
    let eid = entry.id;

    // detail 单行
    let detail = tx
        .query_row(
            "SELECT pattern, variants, form_rule, attaches_to, speech_style, politeness, register, \
             irregular, difficulty, frequency, meaning_brief, usage_scene, notes \
             FROM kr_grammar_detail WHERE entry_id = ?1",
            params![eid],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, Option<String>>(5)?,
                    r.get::<_, Option<String>>(6)?,
                    r.get::<_, i64>(7)?,
                    r.get::<_, Option<i64>>(8)?,
                    r.get::<_, Option<String>>(9)?,
                    r.get::<_, Option<String>>(10)?,
                    r.get::<_, Option<String>>(11)?,
                    r.get::<_, Option<String>>(12)?,
                ))
            },
        )
        .optional()
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let (pattern, variants, form_rule, attaches_to, speech_style, politeness, register, irregular, difficulty, frequency, meaning_brief, usage_scene, notes) =
        match detail {
            Some(d) => d,
            None => {
                return Err(ApiError::schema_mismatch(format!(
                    "条目 {} 缺少 kr_grammar_detail 行",
                    entry.headword
                )));
            }
        };

    // 各子表（均含 ord 排序；scenes 有真实数据 367 条；cautions/collocations 当前源数据
    // 为空表自然返回空 Vec。本模块为 M4 将退役的旧 grammar 读取路径，新数据走 knowledge 包。）
    // 注意：子表 TEXT 列在真实数据中大量为 NULL，一律 COALESCE(…,'') 后再按 String 读取。
    let meanings = {
        let mut stmt = tx
            .prepare("SELECT COALESCE(meaning_zh,''), COALESCE(meaning_ko,''), COALESCE(note,'') FROM kr_grammar_meanings WHERE entry_id=?1 ORDER BY ord, id")
            .map_err(|e| ApiError::internal(e.to_string()))?;
        let rows = stmt
            .query_map(params![eid], row_mean)
            .map_err(|e| ApiError::internal(e.to_string()))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| ApiError::internal(e.to_string()))?
    };

    let usages = {
        let mut stmt = tx
            .prepare("SELECT COALESCE(title,''), COALESCE(usage_zh,''), COALESCE(usage_ko,''), COALESCE(form,'') FROM kr_grammar_usages WHERE entry_id=?1 ORDER BY ord, id")
            .map_err(|e| ApiError::internal(e.to_string()))?;
        let rows = stmt
            .query_map(params![eid], |r| {
                Ok(Usage {
                    title: r.get(0)?,
                    usage_zh: r.get(1)?,
                    usage_ko: r.get(2)?,
                    form: r.get(3)?,
                })
            })
            .map_err(|e| ApiError::internal(e.to_string()))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| ApiError::internal(e.to_string()))?
    };

    let scenes = {
        let mut stmt = tx
            .prepare("SELECT COALESCE(scene,''), COALESCE(note,'') FROM kr_grammar_scenes WHERE entry_id=?1 ORDER BY ord, id")
            .map_err(|e| ApiError::internal(e.to_string()))?;
        let rows = stmt
            .query_map(params![eid], |r| {
                Ok(Scene { scene: r.get(0)?, note: r.get(1)? })
            })
            .map_err(|e| ApiError::internal(e.to_string()))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| ApiError::internal(e.to_string()))?
    };

    let examples = {
        let mut stmt = tx
            .prepare("SELECT COALESCE(ko,''), COALESCE(zh,''), COALESCE(note,'') FROM kr_grammar_examples WHERE entry_id=?1 ORDER BY ord, id")
            .map_err(|e| ApiError::internal(e.to_string()))?;
        let rows = stmt
            .query_map(params![eid], |r| {
                Ok(Example { ko: r.get(0)?, zh: r.get(1)?, note: r.get(2)? })
            })
            .map_err(|e| ApiError::internal(e.to_string()))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| ApiError::internal(e.to_string()))?
    };

    let cautions = {
        let mut stmt = tx
            .prepare("SELECT COALESCE(caution,''), COALESCE(wrong,''), COALESCE(right,'') FROM kr_grammar_cautions WHERE entry_id=?1 ORDER BY ord, id")
            .map_err(|e| ApiError::internal(e.to_string()))?;
        let rows = stmt
            .query_map(params![eid], |r| {
                Ok(Caution {
                    caution: r.get(0)?,
                    wrong: r.get(1)?,
                    right: r.get(2)?,
                })
            })
            .map_err(|e| ApiError::internal(e.to_string()))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| ApiError::internal(e.to_string()))?
    };

    let collocations = {
        let mut stmt = tx
            .prepare("SELECT COALESCE(word,''), COALESCE(note,'') FROM kr_grammar_collocations WHERE entry_id=?1 ORDER BY ord, id")
            .map_err(|e| ApiError::internal(e.to_string()))?;
        let rows = stmt
            .query_map(params![eid], |r| {
                Ok(Collocation { word: r.get(0)?, note: r.get(1)? })
            })
            .map_err(|e| ApiError::internal(e.to_string()))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| ApiError::internal(e.to_string()))?
    };

    // related：已解析目标（关联本集条目），uid 由目标 headword 派生
    let related = {
        let mut stmt = tx
            .prepare(
                "SELECT r.relation, COALESCE(r.note,''), t.headword FROM kr_grammar_related r \
                 JOIN entries t ON t.id = r.related_id WHERE r.entry_id = ?1 ORDER BY r.relation, r.id",
            )
            .map_err(|e| ApiError::internal(e.to_string()))?;
        let rows = stmt
            .query_map(params![eid], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| ApiError::internal(e.to_string()))?;
        let mut out = Vec::new();
        for row in rows {
            let (relation, note, headword) = row.map_err(|e| ApiError::internal(e.to_string()))?;
            out.push(Related {
                uid: uid_of(&headword),
                headword,
                relation,
                note,
            });
        }
        out
    };

    // related_pending：未收录原文，UI 折叠展示
    let related_pending = {
        let mut stmt = tx
            .prepare("SELECT target_text, COALESCE(relation,''), COALESCE(note,'') FROM kr_grammar_related_pending WHERE entry_id=?1 ORDER BY id")
            .map_err(|e| ApiError::internal(e.to_string()))?;
        let rows = stmt
            .query_map(params![eid], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| ApiError::internal(e.to_string()))?;
        let mut out = Vec::new();
        for row in rows {
            let (target_text, relation, note) = row.map_err(|e| ApiError::internal(e.to_string()))?;
            out.push(RelatedPending {
                target_text,
                relation,
                note,
            });
        }
        out
    };

    let code = entry.level_code.clone();
    let (level_label, _rank) = level_meta_for(GRAMMAR_SUBJECT, &code);

    tx.commit().map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(EntryDetail {
        uid: uid.to_string(),
        headword: entry.headword,
        category: entry.category,
        level_code: code,
        level_label,
        tags: entry.tags,
        pattern,
        variants: parse_string_array(variants.as_deref()),
        form_rule: form_rule.unwrap_or_default(),
        attaches_to: parse_string_array(attaches_to.as_deref()),
        speech_style: speech_style.unwrap_or_default(),
        politeness: politeness.unwrap_or_default(),
        register: register.unwrap_or_default(),
        irregular: irregular != 0,
        difficulty: difficulty.map(|d| d as u8),
        frequency: frequency.unwrap_or_default(),
        meaning_brief: meaning_brief.unwrap_or_default(),
        usage_scene: usage_scene.unwrap_or_default(),
        notes: notes.unwrap_or_default(),
        meanings,
        usages,
        scenes,
        examples,
        cautions,
        collocations,
        related,
        related_pending,
    })
}

/// 英语详情：summary/explanation + en_grammar_examples + images（相对路径 → 绝对路径）。
/// `hidden` = 该 entry_uid 在 userdata.image_hidden 中的 img_path 集合（只返回可见图）；
/// `images_dir` = resolve_en_images_dir 结果；为 None 时 images=[]（图解节自动隐藏，不报错）。
pub fn get_detail_en(
    conn: &Connection,
    uid: &str,
    hidden: &HashSet<String>,
    images_dir: Option<&Path>,
) -> CmdResult<EnEntryDetail> {
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let entry = find_entry_by_uid(&tx, EN_GRAMMAR_SUBJECT, uid)?
        .ok_or_else(|| ApiError::entry_not_found(uid.to_string()))?;
    let eid = entry.id;

    let detail = tx
        .query_row(
            "SELECT COALESCE(summary,''), COALESCE(explanation,''), COALESCE(images,'[]') \
             FROM en_grammar_detail WHERE entry_id = ?1",
            params![eid],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            },
        )
        .optional()
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let (summary, explanation, images_json) = match detail {
        Some(d) => d,
        None => {
            return Err(ApiError::schema_mismatch(format!(
                "条目 {} 缺少 en_grammar_detail 行",
                entry.headword
            )));
        }
    };

    // 例句
    let examples = {
        let mut stmt = tx
            .prepare("SELECT COALESCE(en,''), COALESCE(zh,''), COALESCE(note,'') FROM en_grammar_examples WHERE entry_id=?1 ORDER BY ord, id")
            .map_err(|e| ApiError::internal(e.to_string()))?;
        let rows = stmt
            .query_map(params![eid], |r| {
                Ok(EnExample { en: r.get(0)?, zh: r.get(1)?, note: r.get(2)? })
            })
            .map_err(|e| ApiError::internal(e.to_string()))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| ApiError::internal(e.to_string()))?
    };

    // 图片：DB JSON（相对路径）→ 过滤 hidden → join images_dir → 绝对路径。
    // 目录缺失 / 文件缺失时静默跳过（图解节只渲染可见且实际存在的图）。
    let rels: Vec<String> = serde_json::from_str::<Vec<String>>(&images_json)
        .unwrap_or_default()
        .into_iter()
        .filter(|rel| !rel.trim().is_empty() && !hidden.contains(rel.trim()))
        .map(|rel| rel.trim().to_string())
        .collect();
    let images: Vec<EnImage> = match images_dir {
        Some(dir) => rels
            .into_iter()
            .filter_map(|rel| {
                let p = dir.join(&rel);
                if p.is_file() {
                    Some(EnImage { rel, path: p.to_string_lossy().into_owned() })
                } else {
                    None
                }
            })
            .collect(),
        None => Vec::new(),
    };

    let code = entry.level_code.clone();
    let (level_label, _rank) = level_meta_for(EN_GRAMMAR_SUBJECT, &code);

    tx.commit().map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(EnEntryDetail {
        uid: uid.to_string(),
        headword: entry.headword,
        category: entry.category,
        level_code: code,
        level_label,
        tags: entry.tags,
        summary,
        explanation,
        examples,
        images,
    })
}

pub fn db_info(conn: &Connection, path: &Path) -> CmdResult<DbInfo> {
    let entries: i64 = conn
        .query_row("SELECT COUNT(*) FROM entries", [], |r| r.get(0))
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(DbInfo {
        path: path.to_string_lossy().to_string(),
        entries: entries as usize,
        db_updated_at: db_updated_at(conn),
    })
}

pub fn open_and_validate(path: &Path) -> CmdResult<Connection> {
    let conn = open_grammar_ro(path)?;
    ensure_schema(&conn)?;
    Ok(conn)
}

// ---------------------------------------------------------------------------
// 单元测试（get_detail_en 过滤语义；不依赖 tauri 运行环境）
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn seed_en_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE subjects(code TEXT PRIMARY KEY, name TEXT, template TEXT); \
             INSERT INTO subjects(code,name,template) VALUES('en_grammar','英语语法','en_grammar'); \
             CREATE TABLE entries( \
               id INTEGER PRIMARY KEY AUTOINCREMENT, \
               subject_code TEXT NOT NULL, headword TEXT NOT NULL, category TEXT, \
               level TEXT, tags_text TEXT, \
               created_at TEXT DEFAULT (datetime('now')), updated_at TEXT DEFAULT (datetime('now'))); \
             CREATE TABLE en_grammar_detail( \
               entry_id INTEGER PRIMARY KEY, summary TEXT, \
               explanation TEXT NOT NULL DEFAULT '', images TEXT, extra TEXT); \
             CREATE TABLE en_grammar_examples( \
               id INTEGER PRIMARY KEY AUTOINCREMENT, entry_id INTEGER NOT NULL, \
               ord INTEGER DEFAULT 0, en TEXT NOT NULL, zh TEXT, note TEXT);",
        )
        .unwrap();
        conn
    }

    fn insert_en_entry(conn: &Connection, headword: &str, level: &str) -> i64 {
        conn.execute(
            "INSERT INTO entries(subject_code, headword, category, level, tags_text) \
             VALUES(?1,?2,'定语从句',?3,'句法')",
            params![EN_GRAMMAR_SUBJECT, headword, level],
        )
        .unwrap();
        conn.query_row("SELECT id FROM entries WHERE headword=?1", [headword], |r| {
            r.get(0)
        })
        .unwrap()
    }

    fn temp_images_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("knoasis_en_imgs_{}_{}", std::process::id(), tag));
        let book = dir.join("book");
        fs::create_dir_all(&book).unwrap();
        fs::write(book.join("x.jpeg"), b"x").unwrap();
        fs::write(book.join("y.jpeg"), b"y").unwrap();
        dir
    }

    #[test]
    fn subject_routing_by_uid_prefix_and_uid_of_for() {
        // kr：uid_of 与按 kr 学科派生一致（krg: 前缀）
        assert_eq!(uid_of("N마저"), uid_of_for(GRAMMAR_SUBJECT, "N마저"));
        assert_eq!(subject_from_uid(&uid_of("N마저")), GRAMMAR_SUBJECT);
        assert_eq!(subject_from_uid("krg:a1b2c3d4e5f6"), GRAMMAR_SUBJECT);
        // en：en: 前缀（含 -2 撞车补偿后缀仍按 en 路由）
        let en_uid = uid_of_for(EN_GRAMMAR_SUBJECT, "定语从句");
        assert!(en_uid.starts_with("en:"));
        assert_eq!(en_uid.len(), "en:".len() + 12);
        assert_eq!(subject_from_uid(&en_uid), EN_GRAMMAR_SUBJECT);
        assert_eq!(subject_from_uid(&format!("{en_uid}-2")), EN_GRAMMAR_SUBJECT);
        // 未知 → kr（向后兼容）
        assert_eq!(subject_from_uid("legacy:x"), GRAMMAR_SUBJECT);
        assert_eq!(subject_code_of(None), GRAMMAR_SUBJECT);
        assert_eq!(subject_code_of(Some("")), GRAMMAR_SUBJECT);
        assert_eq!(subject_code_of(Some("en-grammar")), EN_GRAMMAR_SUBJECT);
        assert_eq!(subject_code_of(Some("en_grammar")), EN_GRAMMAR_SUBJECT);
        assert_eq!(subject_code_of(Some("kr-grammar")), GRAMMAR_SUBJECT);
        assert_eq!(subject_code_of(Some("unknown")), GRAMMAR_SUBJECT);
    }

    #[test]
    fn get_detail_en_filters_hidden_and_skips_missing_files() {
        let conn = seed_en_db();
        let headword = "定语从句";
        let eid = insert_en_entry(&conn, headword, "III");
        conn.execute(
            "INSERT INTO en_grammar_detail(entry_id, summary, explanation, images) \
             VALUES(?1,'s','e',?2)",
            params![eid, r#"["book/x.jpeg","book/y.jpeg","book/missing.jpeg"]"#],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO en_grammar_examples(entry_id, ord, en, zh, note) \
             VALUES(?1,0,'The book is helpful.','这本书很有帮助。','')",
            params![eid],
        )
        .unwrap();

        let uid = uid_of_for(EN_GRAMMAR_SUBJECT, headword);
        let dir = temp_images_dir("filter");
        let mut hidden: HashSet<String> = HashSet::new();
        hidden.insert("book/y.jpeg".to_string()); // 用户已隐藏

        let dto = get_detail_en(&conn, &uid, &hidden, Some(dir.as_path())).unwrap();
        // y 被 hidden 过滤；missing.jpeg 不存在被跳过；仅 x 可见
        assert_eq!(dto.images.len(), 1);
        assert_eq!(dto.images[0].rel, "book/x.jpeg");
        assert!(dto.images[0].path.ends_with("book/x.jpeg"));
        assert_eq!(dto.level_code, "III");
        assert_eq!(dto.level_label, "高级");
        assert_eq!(dto.summary, "s");
        assert_eq!(dto.explanation, "e");
        assert_eq!(dto.examples.len(), 1);
        assert_eq!(dto.examples[0].en, "The book is helpful.");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn get_detail_en_hidden_survives_reimport_and_empty_dir_is_empty() {
        // 隐藏记录以 uid+rel 为键、与条目行 id / 数据版本解耦：
        // 模拟重导 = 删除旧行 + 同 headword 重新插入（新 entry id），hidden 仍应生效。
        let conn = seed_en_db();
        let headword = "介词 in的用法";
        let eid1 = insert_en_entry(&conn, headword, "I");
        conn.execute(
            "INSERT INTO en_grammar_detail(entry_id, summary, explanation, images) \
             VALUES(?1,'s','e',?2)",
            params![eid1, r#"["book/x.jpeg","book/y.jpeg"]"#],
        )
        .unwrap();

        let uid = uid_of_for(EN_GRAMMAR_SUBJECT, headword);
        let dir = temp_images_dir("reimport");
        let mut hidden: HashSet<String> = HashSet::new();
        hidden.insert("book/x.jpeg".to_string());
        hidden.insert("book/y.jpeg".to_string());
        // 两图全隐藏 → 空
        let dto1 = get_detail_en(&conn, &uid, &hidden, Some(dir.as_path())).unwrap();
        assert_eq!(dto1.images.len(), 0);

        // 模拟重导：删行 + 重建（新 entry id），同 headword → 同 uid
        conn.execute("DELETE FROM en_grammar_detail WHERE entry_id=?1", params![eid1]).unwrap();
        conn.execute("DELETE FROM entries WHERE id=?1", params![eid1]).unwrap();
        let eid2 = insert_en_entry(&conn, headword, "I");
        assert_ne!(eid1, eid2);
        conn.execute(
            "INSERT INTO en_grammar_detail(entry_id, summary, explanation, images) \
             VALUES(?1,'s2','e2',?2)",
            params![eid2, r#"["book/x.jpeg"]"#],
        )
        .unwrap();
        // 同 hidden 集合仍过滤（只新插入了 x，被 hidden → 0）
        let dto2 = get_detail_en(&conn, &uid, &hidden, Some(dir.as_path())).unwrap();
        assert_eq!(dto2.images.len(), 0);

        // images_dir=None（目录不可用）→ images=[]，不报错
        let dto3 = get_detail_en(&conn, &uid, &HashSet::new(), None).unwrap();
        assert_eq!(dto3.images.len(), 0);
        assert_eq!(dto3.summary, "s2");

        fs::remove_dir_all(&dir).ok();
    }
}
