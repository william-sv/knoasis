// Knoasis · knowledge IPC 命令薄壳
//
// 命令表（docs/Knoasis-学科数据组织与第三方接入方案.md §3.8）：
//   knowledge_list_sets      → Vec<SubjectMeta>（meta+template+counts）
//   knowledge_list_entries   → {total, items}（未知 set_id → SET_NOT_FOUND）
//   knowledge_get_entry      → EntryPayload（按 uid 前缀路由；images 过滤 image_hidden）
//   knowledge_reload         → ReloadInfo{sets, entries_total}（重扫目录）
//   学科包管理（Dash 式 Docsets 管理，见 manager.rs）：
//   knowledge_manage_list    → ManageListResult{user_root, sets}（含已停用包）
//   knowledge_set_enabled    → ManageListResult（切换启用/停用后回传最新列表）
//   knowledge_import_set     → ImportReport（校验+复制到用户根）
//   knowledge_remove_set     → RemoveReport（用户包真删 / 内置包退化为停用）
//
// 每次命令都重新扫描两个 knowledge 根（毫秒级），不持有缓存；换包后无需重启。

use serde::Deserialize;
use tauri::{AppHandle, Manager};

use crate::grammar::db::{ApiError, CmdResult};
use crate::knowledge::discovery::{self, ScanOptions, ScanResult};
use crate::knowledge::manager::{
    self, ImportReport, ManageListResult, RemoveReport,
};
use crate::knowledge::reader::{self, EntryFilter};
use crate::knowledge::{EntryPayload, KnowledgeSetMeta, ListEntriesResult, ReloadInfo, SubjectMeta, uid};
use crate::userdata::{self, UserData};

use std::collections::HashSet;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// 入参
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ListEntriesArgs {
    pub set_id: String,
    pub level: Option<String>,
    pub category: Option<String>,
    pub q: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct GetEntryArgs {
    pub uid: String,
}

#[derive(Debug, Deserialize)]
pub struct SetEnabledArgs {
    pub set_id: String,
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct ImportSetArgs {
    /// 导入源：*.kpkg 文件，或 *.knowledgeset 目录
    pub src: String,
}

#[derive(Debug, Deserialize)]
pub struct RemoveSetArgs {
    pub set_id: String,
}

#[derive(Debug, Deserialize)]
pub struct OpenDirArgs {
    /// 待打开目录（必须在用户知识根内）
    pub dir: String,
}

// ---------------------------------------------------------------------------
// 根目录解析（内置根 = 随包资源；用户根 = 应用数据目录）
// ---------------------------------------------------------------------------

/// 内置根：打包后 $RESOURCE/knowledge（随包内置学科包）。
/// 当前发行版不内置任何学科包，该目录通常不存在 → 返回 None，由用户根继续扫描。
fn builtin_root(app: &AppHandle) -> Option<std::path::PathBuf> {
    if let Ok(dir) = app.path().resource_dir() {
        let p = dir.join("knowledge");
        if p.is_dir() {
            return Some(p);
        }
    }
    None
}

/// 用户/第三方根：$APPDATA/com.william.knoasis/knowledge（存在才返回）
fn user_root(app: &AppHandle) -> Option<std::path::PathBuf> {
    let dir = app.path().app_data_dir().ok()?;
    let p = dir.join("knowledge");
    if p.is_dir() {
        Some(p)
    } else {
        None
    }
}

/// 用户/第三方根（不存在则创建；导入/删除/停用需要目录一定存在）
fn ensure_user_root(app: &AppHandle) -> Result<PathBuf, ApiError> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| ApiError::internal(format!("解析应用数据目录失败: {e}")))?;
    let p = dir.join("knowledge");
    std::fs::create_dir_all(&p)
        .map_err(|e| ApiError::internal(format!("创建用户知识根 {} 失败: {e}", p.display())))?;
    Ok(p)
}

/// 内置 template_registry（旧包 template 回退）：随包 grammar/ + dev 仓库 resources/grammar/
fn resolve_registry(app: &AppHandle) -> serde_json::Value {
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(dir) = app.path().resource_dir() {
        candidates.push(dir.join("grammar").join("template_registry.json"));
    }
    candidates.push(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("grammar")
            .join("template_registry.json"),
    );
    for p in candidates.iter() {
        if let Ok(text) = std::fs::read_to_string(p) {
            if let Ok(v) = serde_json::from_str(&text) {
                return v;
            }
        }
    }
    serde_json::Value::Null
}

fn scan_all(app: &AppHandle) -> ScanResult {
    // 已停用的学科包不参与业务扫描（仅在管理面板可见，见 knowledge_manage_list）
    let disabled: HashSet<String> = match user_root(app) {
        Some(root) => manager::load_disabled(&root),
        None => HashSet::new(),
    };
    let options = ScanOptions {
        builtin_root: builtin_root(app),
        user_root: user_root(app),
        registry: resolve_registry(app),
        disabled,
    };
    let res = discovery::scan(&options);
    // 扫描诊断（坏包/坏 schema 等）打 stderr，便于定位第三方包问题
    for w in res.warnings.iter() {
        eprintln!("[knowledge] {w}");
    }
    res
}

fn find_meta<'a>(sets: &'a [KnowledgeSetMeta], set_id: &str) -> Option<&'a KnowledgeSetMeta> {
    discovery::find_set(sets, set_id)
}

// ---------------------------------------------------------------------------
// 命令
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn knowledge_list_sets(app: AppHandle) -> CmdResult<Vec<SubjectMeta>> {
    let res = scan_all(&app);
    Ok(res.sets.iter().map(SubjectMeta::from).collect())
}

#[tauri::command]
pub fn knowledge_list_entries(
    app: AppHandle,
    args: ListEntriesArgs,
) -> CmdResult<ListEntriesResult> {
    let set_id = args.set_id.trim().to_string();
    if set_id.is_empty() {
        return Err(ApiError::set_not_found("set_id 为空"));
    }
    let res = scan_all(&app);
    let meta = find_meta(&res.sets, &set_id)
        .ok_or_else(|| ApiError::set_not_found(format!("未知学科包: {set_id}")))?;
    let filter = EntryFilter {
        level: args.level.as_deref(),
        category: args.category.as_deref(),
        q: args.q.as_deref(),
        limit: args.limit.unwrap_or(5000).min(50000),
    };
    reader::list_entries(meta, &filter)
}

#[tauri::command]
pub fn knowledge_get_entry(app: AppHandle, args: GetEntryArgs) -> CmdResult<EntryPayload> {
    let uid_val = args.uid.trim().to_string();
    if !uid::is_valid_uid(&uid_val) {
        return Err(ApiError::set_not_found(format!("uid 格式非法: {uid_val}")));
    }
    let set_id = uid::set_from_uid(&uid_val)
        .ok_or_else(|| ApiError::set_not_found(format!("uid 无学科前缀: {uid_val}")))?;
    let res = scan_all(&app);
    let meta = find_meta(&res.sets, set_id)
        .ok_or_else(|| ApiError::set_not_found(format!("未知学科包: {set_id}")))?;
    // 读该 entry_uid 的 image_hidden 集合传给 reader 过滤（对齐 grammar en 分支语义）
    let ud = app.state::<UserData>();
    let hidden = userdata::image_hidden_set(&ud, &uid_val)?;
    reader::get_entry(meta, &uid_val, &hidden)
}

#[tauri::command]
pub fn knowledge_reload(app: AppHandle) -> CmdResult<ReloadInfo> {
    let res = scan_all(&app);
    let sets: Vec<SubjectMeta> = res.sets.iter().map(SubjectMeta::from).collect();
    let entries_total = discovery::entries_total(&res.sets);
    Ok(ReloadInfo {
        sets,
        entries_total,
        user_root: user_root(&app)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default(),
    })
}

// ---------------------------------------------------------------------------
// 学科包管理（导入 / 删除 / 启用停用）
// ---------------------------------------------------------------------------

/// 管理面板数据：用户知识根路径 + 全部包（含已停用的，便于重新启用）
fn manage_snapshot(app: &AppHandle) -> CmdResult<ManageListResult> {
    let user = ensure_user_root(app)?;
    let disabled = manager::load_disabled(&user);
    let sets = manager::collect_managed(
        builtin_root(app).as_deref(),
        Some(user.as_path()),
        &resolve_registry(app),
        &disabled,
    );
    Ok(ManageListResult {
        user_root: user.to_string_lossy().into_owned(),
        sets,
    })
}

#[tauri::command]
pub fn knowledge_manage_list(app: AppHandle) -> CmdResult<ManageListResult> {
    manage_snapshot(&app)
}

#[tauri::command]
pub fn knowledge_set_enabled(
    app: AppHandle,
    args: SetEnabledArgs,
) -> CmdResult<ManageListResult> {
    let set_id = args.set_id.trim().to_string();
    if !uid::is_valid_set_id(&set_id) {
        return Err(ApiError::set_not_found(format!("学科包 id 非法: {set_id}")));
    }
    let user = ensure_user_root(&app)?;
    manager::set_enabled(&user, &set_id, args.enabled)
        .map_err(ApiError::internal)?;
    manage_snapshot(&app)
}

#[tauri::command]
pub fn knowledge_import_set(app: AppHandle, args: ImportSetArgs) -> CmdResult<ImportReport> {
    let user = ensure_user_root(&app)?;
    let src = PathBuf::from(args.src.trim());
    manager::import_package(&src, &user).map_err(ApiError::internal)
}

#[tauri::command]
pub fn knowledge_remove_set(app: AppHandle, args: RemoveSetArgs) -> CmdResult<RemoveReport> {
    let set_id = args.set_id.trim().to_string();
    if !uid::is_valid_set_id(&set_id) {
        return Err(ApiError::set_not_found(format!("学科包 id 非法: {set_id}")));
    }
    let user = ensure_user_root(&app)?;
    // 内置是否存在同名包：决定删除后是否回落，或是否退化为「停用」
    let builtin_has = builtin_root(&app)
        .map(|r| r.join(format!("{set_id}.knowledgeset")).is_dir())
        .unwrap_or(false);
    manager::remove_package(&user, &set_id, builtin_has).map_err(ApiError::internal)
}

// ---------------------------------------------------------------------------
// 打开用户知识根目录（替代前端 opener.openPath：绕开 capabilities scope 限制）
// ---------------------------------------------------------------------------

/// 校验目标目录必须位于用户知识根内（规范化后前缀比较）；返回规范化绝对路径。
///
/// 越界（如 /etc、其它用户目录）或目录不存在均返回错误，避免前端被诱导打开任意路径。
fn resolve_within_user_root(user_root: &std::path::Path, dir: &str) -> Result<PathBuf, ApiError> {
    let canon_root = user_root
        .canonicalize()
        .map_err(|e| ApiError::internal(format!("规范化用户知识根失败: {e}")))?;
    let target = PathBuf::from(dir.trim());
    let canon_target = target
        .canonicalize()
        .map_err(|e| ApiError::internal(format!("目录不存在或无法访问: {e}")))?;
    // 必须落在用户知识根之下（含根本身）；字符串前缀比较会误判 /root2，故用 Path::starts_with
    if !canon_target.starts_with(&canon_root) {
        return Err(ApiError::internal(format!(
            "拒绝打开用户知识根之外的目录: {}",
            canon_target.display()
        )));
    }
    Ok(canon_target)
}

/// 在系统文件管理器中打开给定目录（macOS: open / Windows: cmd start / Linux: xdg-open）
fn open_in_file_manager(path: &std::path::Path) -> Result<(), ApiError> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|e| ApiError::internal(format!("无法打开目录: {e}")))?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", ""])
            .arg(path)
            .spawn()
            .map_err(|e| ApiError::internal(format!("无法打开目录: {e}")))?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|e| ApiError::internal(format!("无法打开目录: {e}")))?;
    }
    Ok(())
}

#[tauri::command]
pub fn knowledge_open_dir(app: AppHandle, args: OpenDirArgs) -> CmdResult<()> {
    let user = ensure_user_root(&app)?;
    let target = resolve_within_user_root(&user, &args.dir)?;
    open_in_file_manager(&target)
}

// ---------------------------------------------------------------------------
// 单元测试：路径越界校验（不依赖 tauri 运行环境）
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_within_user_root_allows_root_and_child() {
        let root = std::env::temp_dir().join(format!("knoasis_opendir_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let child = root.join("en-grammar.knowledgeset");
        std::fs::create_dir_all(&child).unwrap();

        // 根本身可打开
        assert!(resolve_within_user_root(&root, &root.to_string_lossy()).is_ok());
        // 根下子目录可打开
        assert!(resolve_within_user_root(&root, &child.to_string_lossy()).is_ok());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn resolve_within_user_root_rejects_outside() {
        let root = std::env::temp_dir().join(format!("knoasis_opendir_out_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();

        let outside = std::env::temp_dir(); // 父级（不在根内）
        let err = resolve_within_user_root(&root, &outside.to_string_lossy()).unwrap_err();
        assert_eq!(err.code, "INTERNAL");

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn resolve_within_user_root_rejects_missing_dir() {
        let root = std::env::temp_dir().join(format!("knoasis_opendir_miss_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();

        let missing = root.join("does-not-exist");
        assert!(resolve_within_user_root(&root, &missing.to_string_lossy()).is_err());

        std::fs::remove_dir_all(&root).ok();
    }

    // QA 独立补充：验证「父级穿越」与「符号链接逃逸」两类越界均被拒绝
    // （工程师原有 3 个单测未覆盖这两条路径；canonicalize 后统一做 starts_with 前缀校验）
    #[test]
    #[cfg(unix)]
    fn qa_resolve_within_user_root_rejects_traversal_and_symlink_escape() {
        use std::os::unix::fs::symlink;

        let base = std::env::temp_dir().join(format!("knoasis_opendir_esc_{}", std::process::id()));
        let root = base.join("knowledge");
        let outside = base.join("outside");
        std::fs::remove_dir_all(&base).ok();
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();

        // 1) 父级穿越：<root>/.. 规范化后落在 base，不在 root 内 → 拒绝
        let parent = root.join("..");
        assert!(
            resolve_within_user_root(&root, &parent.to_string_lossy()).is_err(),
            "父级穿越 <root>/.. 应被拒绝"
        );

        // 2) 符号链接逃逸：<root>/link → outside，canonicalize 解析到 root 之外 → 拒绝
        let link = root.join("link");
        symlink(&outside, &link).unwrap();
        assert!(
            resolve_within_user_root(&root, &link.to_string_lossy()).is_err(),
            "符号链接逃逸应被拒绝"
        );

        // 3) 对照：root 内真实子目录仍可通过
        let inside = root.join("en-grammar.knowledgeset");
        std::fs::create_dir_all(&inside).unwrap();
        assert!(resolve_within_user_root(&root, &inside.to_string_lossy()).is_ok());

        std::fs::remove_dir_all(&base).ok();
    }
}
