// Knoasis · 学科包管理（导入 / 删除 / 停用启用）
//
// 对齐 Dash 的 Docsets 管理语义：
//  - 内置包随应用分发（$RESOURCE/knowledge）：不可删除，只能「停用」（取消勾选）
//  - 用户包（$APPDATA/.../knowledge，含 Studio 导出 / 第三方分发）：可自由导入与删除；
//    删除用户包后若内置仍存在同名包，则自动回落到内置版本
//  - 停用标记落用户知识根 `.disabled.json`（隐藏文件；discovery 只扫 *.knowledgeset 目录，不干扰扫描）
//
// 本文件尽量纯函数化（只收 Path），命令层负责把 AppHandle 解析成根目录并转换错误。

use crate::knowledge::discovery::{self, ScanOptions};
use crate::knowledge::meta::{self, ParsedMeta};
use crate::knowledge::reader;
use crate::knowledge::{KnowledgeSetMeta, SetOrigin};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::fs::File;
use std::path::{Path, PathBuf};

/// 停用标记文件名（位于用户知识根）
pub const DISABLED_FILE: &str = ".disabled.json";

// ---------------------------------------------------------------------------
// DTO
// ---------------------------------------------------------------------------

/// 管理面板一行 = 一个学科包（含已停用的；来源/路径/可否删除）
#[derive(Debug, Clone, Serialize)]
pub struct ManagedSet {
    pub id: String,
    pub name: String,
    pub version: String,
    pub schema_version: i64,
    pub language: String,
    pub kind: String,
    pub color: String,
    pub description: String,
    pub entry_count: usize,
    /// "builtin" | "user"
    pub origin: String,
    pub enabled: bool,
    /// 用户根包可删除；内置包只能停用
    pub removable: bool,
    /// 包目录绝对路径（展示/定位用）
    pub dir: String,
}

/// knowledge_manage_list 返回
#[derive(Debug, Clone, Serialize)]
pub struct ManageListResult {
    /// 用户知识根绝对路径（导入目标目录，前端可展示 + 打开）
    pub user_root: String,
    pub sets: Vec<ManagedSet>,
}

/// 导入结果
#[derive(Debug, Clone, Serialize)]
pub struct ImportReport {
    pub set_id: String,
    pub name: String,
    pub version: String,
    pub entry_count: usize,
    /// true = 覆盖了同名旧包
    pub replaced: bool,
    pub path: String,
}

/// 删除结果（内置包无用户副本时退化为停用）
#[derive(Debug, Clone, Serialize)]
pub struct RemoveReport {
    pub set_id: String,
    /// 真正删除了用户根下的包
    pub removed: bool,
    /// 因无用户副本而改为停用（内置包）
    pub disabled: bool,
    /// 删除用户副本后回落到了内置同名包
    pub fell_back_to_builtin: bool,
}

// ---------------------------------------------------------------------------
// 停用标记
// ---------------------------------------------------------------------------

pub fn disabled_path(user_root: &Path) -> PathBuf {
    user_root.join(DISABLED_FILE)
}

/// 读停用集合（文件缺失/损坏 → 空集合，不报错）
pub fn load_disabled(user_root: &Path) -> HashSet<String> {
    let Ok(text) = fs::read_to_string(disabled_path(user_root)) else {
        return HashSet::new();
    };
    let Ok(v) = serde_json::from_str::<Value>(&text) else {
        return HashSet::new();
    };
    v.as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str())
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default()
}

/// 写停用集合（确定性排序 + pretty，便于人工查看/备份）
pub fn save_disabled(user_root: &Path, set: &HashSet<String>) -> Result<(), String> {
    let mut ids: Vec<String> = set.iter().cloned().collect();
    ids.sort();
    let text = serde_json::to_string_pretty(&ids)
        .map_err(|e| format!("序列化停用列表失败: {e}"))?;
    fs::write(disabled_path(user_root), text)
        .map_err(|e| format!("写 {} 失败: {e}", DISABLED_FILE))
}

/// 切换启用状态，返回最新停用集合
pub fn set_enabled(
    user_root: &Path,
    set_id: &str,
    enabled: bool,
) -> Result<HashSet<String>, String> {
    let mut set = load_disabled(user_root);
    if enabled {
        set.remove(set_id);
    } else {
        set.insert(set_id.to_string());
    }
    save_disabled(user_root, &set)?;
    Ok(set)
}

// ---------------------------------------------------------------------------
// 导入
// ---------------------------------------------------------------------------

/// 校验一个目录是否为可导入的学科包；返回已解析 meta
pub fn validate_importable(src: &Path) -> Result<ParsedMeta, String> {
    if !src.is_dir() {
        return Err(format!("路径不存在或不是目录: {}", src.display()));
    }
    let dir_name = src
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if !dir_name.ends_with(".knowledgeset") {
        return Err(format!(
            "目录名须以 .knowledgeset 结尾（当前: {dir_name}）"
        ));
    }
    let meta_path = src.join("meta.json");
    let text = fs::read_to_string(&meta_path)
        .map_err(|e| format!("读 meta.json 失败: {e}"))?;
    let parsed = meta::parse_meta_json(&text)?;
    if !meta::dir_matches_set_id(&dir_name, &parsed.set_id) {
        return Err(format!(
            "目录名 {dir_name} 与 meta.id {} 不一致",
            parsed.set_id
        ));
    }
    if !src.join("knowledge.db").is_file() {
        return Err("包内缺少 knowledge.db".to_string());
    }
    Ok(parsed)
}

/// 递归复制目录（扁平实现：目录递归、文件 copy）
pub fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| format!("创建目录 {} 失败: {e}", dst.display()))?;
    let entries = fs::read_dir(src).map_err(|e| format!("读取 {} 失败: {e}", src.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("遍历 {} 失败: {e}", src.display()))?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir_all(&from, &to)?;
        } else {
            fs::copy(&from, &to)
                .map_err(|e| format!("复制 {} 失败: {e}", from.display()))?;
        }
    }
    Ok(())
}

fn same_path(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

fn tmp_dir_name(set_id: &str) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!(".tmp-import-{set_id}-{now}")
}

/// 导入源规整：把任意导入源解析成「.knowledgeset 目录」+ 是否需要事后清理的临时目录。
///  - 目录（含 .knowledgeset 与任意目录）→ 原样返回，无需清理
///  - .kpkg 文件 → 解包到临时目录 `<temp>/<stem>.knowledgeset`，返回该目录 + 清理标记
///  - 其它 → 报错
fn resolve_import_src(src: &Path) -> Result<(PathBuf, Option<PathBuf>), String> {
    if src.is_file() {
        let ext = src
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if ext != "kpkg" {
            return Err(
                "不支持的导入文件类型（请选择 .kpkg 文件或 .knowledgeset 目录）".to_string(),
            );
        }
        let stem = src
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| "无法解析 .kpkg 文件名".to_string())?
            .to_string();
        let tmp = std::env::temp_dir().join(format!("{stem}.knowledgeset"));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).map_err(|e| format!("创建解包临时目录失败: {e}"))?;
        unzip_to_dir(src, &tmp)?;
        return Ok((tmp.clone(), Some(tmp)));
    }
    if src.is_dir() {
        return Ok((src.to_path_buf(), None));
    }
    Err(format!("导入源不存在: {}", src.display()))
}

/// 将 .kpkg（内部 zip）解包到 dest，路径做 zip-slip 防护（跳过 enclosed_name 越界项）。
fn unzip_to_dir(kpkg: &Path, dest: &Path) -> Result<(), String> {
    let file = File::open(kpkg).map_err(|e| format!("打开 .kpkg 失败: {e}"))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("解析 .kpkg 失败（不是有效的包文件）: {e}"))?;
    for i in 0..archive.len() {
        let mut zf = archive
            .by_index(i)
            .map_err(|e| format!("读取 .kpkg 条目失败: {e}"))?;
        let rel = match zf.enclosed_name() {
            Some(p) => p.to_path_buf(),
            None => continue, // zip-slip 防护：跳过非法路径
        };
        let outpath = dest.join(&rel);
        if zf.is_dir() {
            fs::create_dir_all(&outpath).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = outpath.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut outfile = File::create(&outpath).map_err(|e| e.to_string())?;
            std::io::copy(&mut zf, &mut outfile).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// 导入学科包到用户知识根：校验 → 复制到临时目录 → 副本自检 → 替换目标。
/// 任何一步失败都会清掉临时目录并返回错误（不留半包）。
///
/// 入参 `src` 支持两种形态：
///  - 目录 `<id>.knowledgeset`（旧式，仍兼容）
///  - 单个文件 `<id>.kpkg`（Studio 导出，内部为 zip；自动解包后再走目录逻辑）
pub fn import_package(src: &Path, user_root: &Path) -> Result<ImportReport, String> {
    let (dir, extract_tmp) = resolve_import_src(src)?;
    let report = import_package_from_dir(&dir, user_root);
    // 若源是 .kpkg 解出的临时目录，无论成败都清理，避免留下半包
    if let Some(t) = &extract_tmp {
        let _ = fs::remove_dir_all(t);
    }
    report
}

/// `import_package` 的核心：入参已是「.knowledgeset 目录」，执行校验与安装。
fn import_package_from_dir(dir: &Path, user_root: &Path) -> Result<ImportReport, String> {
    let parsed = validate_importable(dir)?;
    fs::create_dir_all(user_root)
        .map_err(|e| format!("创建用户知识根 {} 失败: {e}", user_root.display()))?;

    let dst = user_root.join(format!("{}.knowledgeset", parsed.set_id));
    if same_path(dir, &dst) {
        return Err("该学科包已安装在用户目录，无需导入".to_string());
    }

    let replaced = dst.exists();
    let tmp = user_root.join(tmp_dir_name(&parsed.set_id));
    let _ = fs::remove_dir_all(&tmp);
    if let Err(e) = copy_dir_all(dir, &tmp) {
        let _ = fs::remove_dir_all(&tmp);
        return Err(e);
    }
    // 副本自检：DB 可打开且能算计数
    let counts = match reader::counts_from_db(&tmp.join("knowledge.db")) {
        Ok(c) => c,
        Err(e) => {
            let _ = fs::remove_dir_all(&tmp);
            return Err(format!("导入后自检失败（已回滚）: {e}"));
        }
    };
    if replaced {
        fs::remove_dir_all(&dst)
            .map_err(|e| format!("移除旧版本失败（新包未安装）: {e}"))?;
    }
    fs::rename(&tmp, &dst).map_err(|e| {
        let _ = fs::remove_dir_all(&tmp);
        format!("安装学科包失败: {e}")
    })?;

    // 导入视为启用：清掉可能残留的停用标记
    let mut disabled = load_disabled(user_root);
    if disabled.remove(&parsed.set_id) {
        save_disabled(user_root, &disabled)?;
    }

    Ok(ImportReport {
        set_id: parsed.set_id,
        name: parsed.name,
        version: parsed.version,
        entry_count: counts.total,
        replaced,
        path: dst.to_string_lossy().into_owned(),
    })
}

// ---------------------------------------------------------------------------
// 删除
// ---------------------------------------------------------------------------

/// 删除学科包：
///  - 用户根存在副本 → 真删除（若有内置同名包则回落）
///  - 仅内置存在 → 退化为停用（返回 disabled=true）
pub fn remove_package(
    user_root: &Path,
    set_id: &str,
    builtin_has: bool,
) -> Result<RemoveReport, String> {
    let dir = user_root.join(format!("{set_id}.knowledgeset"));
    if dir.exists() {
        fs::remove_dir_all(&dir)
            .map_err(|e| format!("删除 {} 失败: {e}", dir.display()))?;
        // 清掉可能残留的停用标记（包已不存在）
        let mut disabled = load_disabled(user_root);
        if disabled.remove(set_id) {
            save_disabled(user_root, &disabled)?;
        }
        return Ok(RemoveReport {
            set_id: set_id.to_string(),
            removed: true,
            disabled: false,
            fell_back_to_builtin: builtin_has,
        });
    }
    if builtin_has {
        let mut disabled = load_disabled(user_root);
        disabled.insert(set_id.to_string());
        save_disabled(user_root, &disabled)?;
        return Ok(RemoveReport {
            set_id: set_id.to_string(),
            removed: false,
            disabled: true,
            fell_back_to_builtin: false,
        });
    }
    Err(format!("未找到学科包: {set_id}"))
}

// ---------------------------------------------------------------------------
// 管理列表
// ---------------------------------------------------------------------------

/// 构造管理视图数据：扫描全部包（**含已停用**），逐条标注来源/可删/启用。
/// 不传 disabled 过滤，保证被停用的包仍能在管理面板里被重新启用。
pub fn collect_managed(
    builtin_root: Option<&Path>,
    user_root: Option<&Path>,
    registry: &Value,
    disabled: &HashSet<String>,
) -> Vec<ManagedSet> {
    let res = discovery::scan(&ScanOptions {
        builtin_root: builtin_root.map(|p| p.to_path_buf()),
        user_root: user_root.map(|p| p.to_path_buf()),
        registry: registry.clone(),
        disabled: HashSet::new(),
    });
    let mut sets: Vec<ManagedSet> = res
        .sets
        .iter()
        .map(|m| to_managed(m, disabled))
        .collect();
    sets.sort_by(|a, b| a.id.cmp(&b.id));
    sets
}

fn to_managed(m: &KnowledgeSetMeta, disabled: &HashSet<String>) -> ManagedSet {
    ManagedSet {
        id: m.set_id.clone(),
        name: m.name.clone(),
        version: m.version.clone(),
        schema_version: m.schema_version,
        language: m.language.clone(),
        kind: m.kind.clone(),
        color: m.color.clone(),
        description: m.description.clone(),
        entry_count: m.counts.total,
        origin: m.origin.as_str().to_string(),
        enabled: !disabled.contains(&m.set_id),
        removable: m.origin == SetOrigin::User,
        dir: m
            .db_path
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default(),
    }
}

// ---------------------------------------------------------------------------
// 单元测试（临时目录；不依赖 tauri 运行环境）
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{params, Connection};
    use std::collections::HashSet;

    fn temp_root(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "knoasis_manager_{}_{tag}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_pkg(root: &Path, set_id: &str, version: &str, rows: &[(&str, &str)]) -> PathBuf {
        let dir = root.join(format!("{set_id}.knowledgeset"));
        fs::create_dir_all(&dir).unwrap();
        let meta = serde_json::json!({
            "schema_version": 1,
            "id": set_id,
            "name": format!("{set_id} 名称"),
            "version": version,
            "language": "en",
            "kind": "grammar",
            "description": "测试包",
            "color": "#2563EB",
            "levels": {
                "system": "stage",
                "label": "难度",
                "values": [ { "code": "A", "label": "A 级", "rank": 1 } ]
            },
            "types": [ { "value": "grammar", "label": "语法" } ]
        });
        fs::write(dir.join("meta.json"), serde_json::to_string_pretty(&meta).unwrap()).unwrap();
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
        for (i, (hw, lv)) in rows.iter().enumerate() {
            conn.execute(
                "INSERT INTO entries(id, uid, headword, category, level, tags, summary) \
                 VALUES(?1, ?2, ?3, '类', ?4, '', 's')",
                params![
                    (i + 1) as i64,
                    crate::knowledge::uid::derive(set_id, hw),
                    hw,
                    lv
                ],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO entry_detail(entry_id, content) VALUES(?1, '{}')",
                params![(i + 1) as i64],
            )
            .unwrap();
        }
        drop(conn);
        dir
    }

    #[test]
    fn disabled_roundtrip() {
        let root = temp_root("disabled");
        assert!(load_disabled(&root).is_empty());
        let set = set_enabled(&root, "en-grammar", false).unwrap();
        assert!(set.contains("en-grammar"));
        assert!(load_disabled(&root).contains("en-grammar"));
        let set2 = set_enabled(&root, "en-grammar", true).unwrap();
        assert!(!set2.contains("en-grammar"));
        assert!(load_disabled(&root).is_empty());
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn validate_rejects_bad_packages() {
        let root = temp_root("bad");
        // 非目录
        assert!(validate_importable(&root.join("nope.knowledgeset")).is_err());
        // 目录名不以 .knowledgeset 结尾
        let plain = root.join("plain-dir");
        fs::create_dir_all(&plain).unwrap();
        assert!(validate_importable(&plain).is_err());
        // 缺 meta.json
        let no_meta = root.join("x.knowledgeset");
        fs::create_dir_all(&no_meta).unwrap();
        assert!(validate_importable(&no_meta).is_err());
        // 目录名与 id 不一致
        let mism = root.join("other.knowledgeset");
        fs::create_dir_all(&mism).unwrap();
        fs::write(
            mism.join("meta.json"),
            r#"{"schema_version":1,"id":"some-id","name":"n","version":"1.0.0","language":"en","kind":"grammar","description":"d"}"#,
        )
        .unwrap();
        assert!(validate_importable(&mism).is_err());
        // 缺 knowledge.db
        let no_db = root.join("nodb.knowledgeset");
        fs::create_dir_all(&no_db).unwrap();
        fs::write(
            no_db.join("meta.json"),
            r#"{"schema_version":1,"id":"nodb","name":"n","version":"1.0.0","language":"en","kind":"grammar","description":"d"}"#,
        )
        .unwrap();
        assert!(validate_importable(&no_db).is_err());

        // 合法
        let ok = write_pkg(&root, "ok-set", "1.0.0", &[("a", "A")]);
        assert!(validate_importable(&ok).is_ok());
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn import_then_replace_then_remove() {
        let src_root = temp_root("src");
        let user_root = temp_root("user");
        let src = write_pkg(&src_root, "demo-set", "1.0.0", &[("a", "A"), ("b", "A")]);

        let rep = import_package(&src, &user_root).unwrap();
        assert_eq!(rep.set_id, "demo-set");
        assert!(!rep.replaced);
        assert_eq!(rep.entry_count, 2);
        assert!(user_root.join("demo-set.knowledgeset").join("knowledge.db").is_file());

        // 再次导入（源目录不同）→ replaced
        let src2_root = temp_root("src2");
        let src2 = write_pkg(&src2_root, "demo-set", "2.0.0", &[("a", "A")]);
        let rep2 = import_package(&src2, &user_root).unwrap();
        assert!(rep2.replaced);
        assert_eq!(rep2.version, "2.0.0");
        assert_eq!(rep2.entry_count, 1);
        // 临时目录已清理
        assert!(
            fs::read_dir(&user_root)
                .unwrap()
                .flatten()
                .all(|e| !e.file_name().to_string_lossy().starts_with(".tmp-import-")),
            "临时导入目录未清理"
        );

        // 删除（无内置同名）→ removed=true，不回落
        let rm = remove_package(&user_root, "demo-set", false).unwrap();
        assert!(rm.removed);
        assert!(!rm.fell_back_to_builtin);
        assert!(!user_root.join("demo-set.knowledgeset").exists());

        fs::remove_dir_all(&src_root).ok();
        fs::remove_dir_all(&src2_root).ok();
        fs::remove_dir_all(&user_root).ok();
    }

    #[test]
    fn remove_builtin_only_disables_and_reports_fallback() {
        let user_root = temp_root("user2");
        // 仅内置存在 → 删除退化为停用
        let rm = remove_package(&user_root, "kr-grammar", true).unwrap();
        assert!(!rm.removed);
        assert!(rm.disabled);
        assert!(load_disabled(&user_root).contains("kr-grammar"));

        // 用户副本存在 + 内置存在 → 真删且回落
        let src_root = temp_root("src3");
        let src = write_pkg(&src_root, "kr-grammar", "1.0.0", &[("x", "A")]);
        import_package(&src, &user_root).unwrap();
        let rm2 = remove_package(&user_root, "kr-grammar", true).unwrap();
        assert!(rm2.removed);
        assert!(rm2.fell_back_to_builtin);
        assert!(load_disabled(&user_root).is_empty());

        // 不存在的包
        assert!(remove_package(&user_root, "ghost", false).is_err());

        fs::remove_dir_all(&src_root).ok();
        fs::remove_dir_all(&user_root).ok();
    }

    #[test]
    fn managed_list_marks_origin_enabled_and_scan_filters_disabled() {
        let builtin = temp_root("mb");
        let user = temp_root("mu");
        write_pkg(&builtin, "kr-grammar", "1.0.0", &[("a", "A")]);
        write_pkg(&user, "en-grammar", "1.2.0", &[("b", "A"), ("c", "A")]);

        let empty: HashSet<String> = HashSet::new();
        let sets = collect_managed(Some(&builtin), Some(&user), &Value::Null, &empty);
        assert_eq!(sets.len(), 2);
        assert_eq!(sets[0].id, "en-grammar");
        assert_eq!(sets[0].origin, "user");
        assert!(sets[0].removable);
        assert!(sets[0].enabled);
        assert_eq!(sets[0].entry_count, 2);
        assert_eq!(sets[1].origin, "builtin");
        assert!(!sets[1].removable);

        // 停用内置包 → 管理列表仍可见但 enabled=false；普通扫描不再包含它
        set_enabled(&user, "kr-grammar", false).unwrap();
        let disabled = load_disabled(&user);
        let sets2 = collect_managed(Some(&builtin), Some(&user), &Value::Null, &disabled);
        assert_eq!(sets2.len(), 2, "停用的包仍应在管理面板可见");
        let kr = sets2.iter().find(|s| s.id == "kr-grammar").unwrap();
        assert!(!kr.enabled);

        let scan = discovery::scan(&ScanOptions {
            builtin_root: Some(builtin.clone()),
            user_root: Some(user.clone()),
            registry: Value::Null,
            disabled,
        });
        assert_eq!(scan.sets.len(), 1);
        assert_eq!(scan.sets[0].set_id, "en-grammar");

        fs::remove_dir_all(&builtin).ok();
        fs::remove_dir_all(&user).ok();
    }
}
