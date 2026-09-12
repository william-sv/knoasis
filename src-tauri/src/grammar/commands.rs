// Knoasis · grammar 只读命令（grammar_list_sets / list_entries / get_detail / reload_db）
//
// 契约见 docs/Knoasis-英语语法接入设计.md §3（多学科泛化，kr JSON 契约冻结）。
// 每个命令都现开现关只读连接，不持有长连接；DB 文件被替换后无需重启，直接读到新数据。
//
// grammar_get_detail 按 uid 前缀路由：
//  - krg: → db::get_detail_kr（原逻辑原样，序列化逐字节不变）
//  - en:  → 读 userdata.image_hidden 的 hidden 集合 → db::get_detail_en 过滤 images

use serde::Deserialize;
use tauri::AppHandle;
use tauri::Manager;

use super::db::*;
use crate::userdata::{self, UserData};

// ---------------------------------------------------------------------------
// 入参
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ListEntriesArgs {
    #[serde(rename = "set_id")]
    pub set_id: Option<String>,
    pub level: Option<String>,
    pub category: Option<String>,
    pub q: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct GetDetailArgs {
    pub uid: String,
}

#[derive(Debug, Deserialize)]
pub struct ReloadDbArgs {
    pub path: Option<String>,
}

// ---------------------------------------------------------------------------
// 模板注册表解析：优先与 grammar.db 同目录，其次随包资源
// ---------------------------------------------------------------------------

fn resolve_template_registry(app: &AppHandle, db_dir: &std::path::Path) -> serde_json::Value {
    let candidates = [
        db_dir.join("template_registry.json"),
        app.path()
            .resource_dir()
            .map(|d| d.join("grammar").join("template_registry.json"))
            .unwrap_or_default(),
    ];
    for p in candidates.iter() {
        if let Ok(text) = std::fs::read_to_string(p) {
            if let Ok(v) = serde_json::from_str(&text) {
                return v;
            }
        }
    }
    serde_json::Value::Null
}

fn open_and_meta(app: &AppHandle) -> CmdResult<(std::path::PathBuf, rusqlite::Connection, serde_json::Value)> {
    let path = resolve_grammar_db(app)?;
    let conn = open_and_validate(&path)?;
    let registry = resolve_template_registry(app, path.parent().unwrap_or(std::path::Path::new("")));
    Ok((path, conn, registry))
}

// ---------------------------------------------------------------------------
// 命令
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn grammar_list_sets(app: AppHandle) -> CmdResult<Vec<SubjectMeta>> {
    let (_path, conn, registry) = open_and_meta(&app)?;
    list_sets(&conn, &registry)
}

#[tauri::command]
pub fn grammar_list_entries(
    app: AppHandle,
    args: ListEntriesArgs,
) -> CmdResult<ListEntriesResult> {
    let (_path, conn, _registry) = open_and_meta(&app)?;
    // set_id → subject（en-grammar/en_grammar → en_grammar；缺省 → kr_grammar 向后兼容）
    let subject = subject_code_of(args.set_id.as_deref());
    list_entries(
        &conn,
        subject,
        args.set_id.as_deref(),
        args.level.as_deref(),
        args.category.as_deref(),
        args.q.as_deref(),
        args.limit.unwrap_or(5000).min(50000),
    )
}

#[tauri::command]
pub fn grammar_get_detail(app: AppHandle, args: GetDetailArgs) -> CmdResult<EntryDetailPayload> {
    let (_path, conn, _registry) = open_and_meta(&app)?;
    let subject = subject_from_uid(&args.uid);
    if subject == EN_GRAMMAR_SUBJECT {
        // en 分支：command 层读该 entry_uid 的 hidden img_path 集合，传给 db 过滤；
        // content DTO 只返回「用户可见」图（隐藏过滤位置 = Rust 读出口，见设计 §3.4/§4.8）。
        let ud = app.state::<UserData>();
        let hidden = userdata::image_hidden_set(&ud, &args.uid)?;
        let images_dir = resolve_en_images_dir(&app);
        get_detail_en(&conn, &args.uid, &hidden, images_dir.as_deref())
            .map(EntryDetailPayload::En)
    } else {
        get_detail_kr(&conn, &args.uid).map(EntryDetailPayload::Kr)
    }
}

#[tauri::command]
pub fn grammar_reload_db(app: AppHandle, args: ReloadDbArgs) -> CmdResult<DbInfo> {
    let path = match args.path {
        Some(p) if !p.trim().is_empty() => {
            let pb = std::path::PathBuf::from(p);
            if pb.is_file() {
                pb
            } else {
                return Err(ApiError::db_unavailable(format!(
                    "指定的 grammar.db 不存在：{}",
                    pb.to_string_lossy()
                )));
            }
        }
        _ => resolve_grammar_db(&app)?,
    };
    let conn = open_and_validate(&path)?;
    db_info(&conn, &path)
}
