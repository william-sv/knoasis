// Knoasis · knowledge 发现机制（扫描两知识根读 meta.json → KnowledgeSetMeta）
//
// 规则（docs/Knoasis-学科数据组织与第三方接入方案.md §3.4）：
//  1. 两根：随包内置根 $RESOURCE/knowledge（dev 下回退仓库 resources/knowledge）+
//     用户/第三方根 $APPDATA/.../knowledge
//  2. 遍历根下 `*.knowledgeset/` 目录读 meta.json；解析失败 / 结构非法 → 跳过并记 warning
//  3. 同 set_id 冲突：版本高者胜；版本相同用户根胜
//  4. counts（total/by_level）打开包内 knowledge.db 只读计算
//
// 本文件尽量纯函数化：命令层负责把 tauri AppHandle 解析成根目录列表，本文件只收 Path。

use crate::knowledge::meta::{self, ParsedMeta};
use crate::knowledge::reader;
use crate::knowledge::{KnowledgeSetMeta, SetOrigin};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// 扫描入参：可用的根目录（存在才传 Some）
#[derive(Debug, Default, Clone)]
pub struct ScanOptions {
    pub builtin_root: Option<PathBuf>,
    pub user_root: Option<PathBuf>,
    /// 内置 template_registry 全文（旧包 template 回退用；可为 Null）
    pub registry: Value,
    /// 已停用学科包 id（管理面板「停用」落 .disabled.json）；非空时这些包不进入扫描结果
    pub disabled: std::collections::HashSet<String>,
}

/// 扫描结果：有效包（已做冲突合并 + counts 计算）与警告列表
#[derive(Debug)]
pub struct ScanResult {
    pub sets: Vec<KnowledgeSetMeta>,
    pub warnings: Vec<String>,
}

/// 解析一个 set 目录为 meta + template（不含 counts/db 打开）；失败返回人类可读原因
fn load_parsed(set_dir: &Path, registry: &Value) -> Result<(ParsedMeta, Value), String> {
    let dir_name = set_dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if !dir_name.ends_with(".knowledgeset") {
        return Err("目录名不以 .knowledgeset 结尾".to_string());
    }
    let meta_path = set_dir.join("meta.json");
    let text = std::fs::read_to_string(&meta_path)
        .map_err(|e| format!("读 meta.json 失败: {e}"))?;
    let parsed = meta::parse_meta_json(&text)?;
    // 目录名 = {id}.knowledgeset
    if !meta::dir_matches_set_id(&dir_name, &parsed.set_id) {
        return Err(format!(
            "目录名 {dir_name} 与 meta.id {} 不一致",
            parsed.set_id
        ));
    }
    // template 解析（包内 > registry > 默认）
    let template = meta::resolve_template(set_dir, &parsed.set_id, &parsed.name, registry);
    Ok((parsed, template))
}

/// 把一个已解析候选构造成 KnowledgeSetMeta；knowledge.db 不可用返回 Err（跳过包）
fn build_meta(parsed: ParsedMeta, template: Value, origin: SetOrigin, set_dir: &Path) -> Result<KnowledgeSetMeta, String> {
    let db_path = set_dir.join("knowledge.db");
    let counts = reader::counts_from_db(&db_path)?;
    let assets_dir = {
        let a = set_dir.join("assets");
        if a.is_dir() {
            Some(a)
        } else {
            None
        }
    };
    Ok(KnowledgeSetMeta {
        set_id: parsed.set_id,
        name: parsed.name,
        version: parsed.version,
        schema_version: parsed.schema_version,
        language: parsed.language,
        kind: parsed.kind,
        color: parsed.color,
        description: parsed.description,
        level_system: parsed.level_system,
        level_system_label: parsed.level_system_label,
        levels: parsed.levels,
        types: parsed.types,
        template,
        counts,
        origin,
        db_path,
        assets_dir,
    })
}

/// 收集单个根 → 候选 Vec<(set_id, meta)>（同根内重复按版本高者留）
fn collect_root(
    root: &Path,
    origin: SetOrigin,
    registry: &Value,
) -> (Vec<KnowledgeSetMeta>, Vec<String>) {
    let mut warnings: Vec<String> = Vec::new();
    let mut by_id: HashMap<String, KnowledgeSetMeta> = HashMap::new();
    // 根目录缺失或不是目录：静默跳过（不报错、不 panic、不产生假告警）。
    // 内置根删除内置包后可能整体不存在（dev 回退仓库路径 / 打包资源目录两者皆可能缺失），
    // 这是正常状态，交给用户根继续扫描即可。
    if !root.is_dir() {
        return (Vec::new(), warnings);
    }
    let entries = match std::fs::read_dir(root) {
        Ok(e) => e,
        Err(e) => {
            warnings.push(format!("扫描根 {} 失败: {e}", root.display()));
            return (Vec::new(), warnings);
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(file_name) = path.file_name() else { continue };
        let dir_name = file_name.to_string_lossy().into_owned();
        if !dir_name.ends_with(".knowledgeset") {
            continue;
        }
        let (parsed, template) = match load_parsed(&path, registry) {
            Ok(p) => p,
            Err(e) => {
                warnings.push(format!("[{}] {dir_name} 跳过: {e}", origin.as_str()));
                continue;
            }
        };
        let meta = match build_meta(parsed, template, origin, &path) {
            Ok(m) => m,
            Err(e) => {
                warnings.push(format!(
                    "[{}] {dir_name} 跳过（knowledge.db 不可用）: {e}",
                    origin.as_str()
                ));
                continue;
            }
        };
        let key = meta.set_id.clone();
        match by_id.get_mut(&key) {
            None => {
                by_id.insert(key, meta);
            }
            Some(existing) => {
                if version_cmp(&meta.version, &existing.version) == std::cmp::Ordering::Greater {
                    *existing = meta;
                }
            }
        }
    }
    let mut sets: Vec<KnowledgeSetMeta> = by_id.into_values().collect();
    sets.sort_by(|a, b| a.set_id.cmp(&b.set_id));
    (sets, warnings)
}

/// 版本比较（semver 宽松）：按 `.` 数字分段比较，失败时 lexicographic 兜底。
/// 说明：只取每段前导数字的宽松规则仅影响未来第三方包「更高版本」判定（阶段 1 无真实升级场景）；
/// 同 set_id 冲突语义仍为「版本高者胜、同版本用户根胜」。
fn version_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    parse_version(a).cmp(&parse_version(b))
}

fn parse_version(v: &str) -> Vec<u64> {
    v.split('.')
        .map(|part| {
            part.chars()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse()
                .unwrap_or(0)
        })
        .collect()
}

/// 主要扫描入口（纯函数）。options 中不存在的根自动跳过。
/// 内置根先入 map；用户根按「版本高胜 / 同版本用户胜」覆盖。
pub fn scan(options: &ScanOptions) -> ScanResult {
    let mut warnings: Vec<String> = Vec::new();
    let mut winners: HashMap<String, KnowledgeSetMeta> = HashMap::new();

    if let Some(root) = &options.builtin_root {
        let (sets, ws) = collect_root(root, SetOrigin::Builtin, &options.registry);
        warnings.extend(ws);
        for m in sets {
            let key = m.set_id.clone();
            match winners.get_mut(&key) {
                None => {
                    winners.insert(key, m);
                }
                Some(cur) => {
                    if version_cmp(&m.version, &cur.version) == std::cmp::Ordering::Greater {
                        *cur = m;
                    }
                }
            }
        }
    }

    if let Some(root) = &options.user_root {
        let (sets, ws) = collect_root(root, SetOrigin::User, &options.registry);
        warnings.extend(ws);
        for m in sets {
            let key = m.set_id.clone();
            match winners.get_mut(&key) {
                None => {
                    winners.insert(key, m);
                }
                Some(cur) => {
                    let user_winner = version_cmp(&m.version, &cur.version)
                        == std::cmp::Ordering::Greater
                        || (version_cmp(&m.version, &cur.version) == std::cmp::Ordering::Equal
                            && cur.origin == SetOrigin::Builtin);
                    if user_winner {
                        *cur = m;
                    }
                }
            }
        }
    }

    // 确定性输出：按 set_id 排序（已停用的包在此剔除：不进学科列表/搜索/条目）
    let mut sets: Vec<KnowledgeSetMeta> = winners
        .into_values()
        .filter(|m| !options.disabled.contains(&m.set_id))
        .collect();
    sets.sort_by(|a, b| a.set_id.cmp(&b.set_id));
    ScanResult { sets, warnings }
}

/// 从扫描结果按 set_id 取元信息
pub fn find_set<'a>(
    sets: &'a [KnowledgeSetMeta],
    set_id: &str,
) -> Option<&'a KnowledgeSetMeta> {
    sets.iter().find(|m| m.set_id == set_id)
}

/// 便捷计数：跨所有已发现包求和
pub fn entries_total(sets: &[KnowledgeSetMeta]) -> usize {
    sets.iter().map(|m| m.counts.total).sum()
}

// ---------------------------------------------------------------------------
// 单元测试（临时目录最小合法/非法包；不依赖 tauri）
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::{SetOrigin, TypeOpt, LevelOpt};
    use rusqlite::{params, Connection};
    use std::fs;

    fn temp_root(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "knoasis_discovery_{}_{tag}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_minimal_package(
        root: &Path,
        set_id: &str,
        version: &str,
        entries: &[(&str, &str)], // (headword, level)
    ) -> PathBuf {
        let dir = root.join(format!("{set_id}.knowledgeset"));
        fs::create_dir_all(&dir).unwrap();
        let meta = serde_json::json!({
            "schema_version": 1,
            "id": set_id,
            "name": set_id,
            "version": version,
            "language": "ko",
            "kind": "grammar",
            "description": "测试包",
            "color": "#D97706",
            "levels": {
                "system": "topik",
                "label": "TOPIK",
                "values": [
                    { "code": "I", "label": "TOPIK I", "rank": 1 },
                    { "code": "II", "label": "TOPIK II", "rank": 2 }
                ]
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
        for (i, (hw, lv)) in entries.iter().enumerate() {
            conn.execute(
                "INSERT INTO entries(id, uid, headword, category, level, tags, summary) \
                 VALUES(?1, ?2, ?3, '测试', ?4, '', 's')",
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
    fn scan_finds_valid_and_skips_invalid_package() {
        let root = temp_root("basic");
        // 合法包
        write_minimal_package(&root, "kr-grammar", "1.0.0", &[("N마저", "II"), ("-고", "I")]);
        // 非法包：meta.json 坏 JSON
        let bad = root.join("bad-set.knowledgeset");
        fs::create_dir_all(&bad).unwrap();
        fs::write(bad.join("meta.json"), "{ not json").unwrap();

        let opts = ScanOptions {
            builtin_root: Some(root.clone()),
            user_root: None,
            registry: Value::Null,
            disabled: std::collections::HashSet::new(),
        };
        let res = scan(&opts);
        assert_eq!(res.sets.len(), 1);
        assert_eq!(res.sets[0].set_id, "kr-grammar");
        assert_eq!(res.sets[0].counts.total, 2);
        assert_eq!(res.sets[0].counts.by_level.get("I"), Some(&1));
        assert_eq!(res.sets[0].counts.by_level.get("II"), Some(&1));
        assert!(
            res.warnings.iter().any(|w| w.contains("bad-set")),
            "warnings={:?}",
            res.warnings
        );

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn user_root_wins_on_same_version_and_skips_schema_too_high() {
        let builtin = temp_root("builtin");
        let user = temp_root("user");

        // 内置 1.0.0
        write_minimal_package(&builtin, "kr-grammar", "1.0.0", &[("N마저", "II")]);
        // 用户 1.0.0（同版本 → 用户胜）
        let user_dir = write_minimal_package(&user, "kr-grammar", "1.0.0", &[("아이", "I")]);

        // schema_version 过高 → 跳过
        let high = user.join("too-new.knowledgeset");
        fs::create_dir_all(&high).unwrap();
        fs::write(
            high.join("meta.json"),
            r#"{ "schema_version": 999, "id": "too-new", "name": "x", "version": "9.0.0", "language": "ko", "kind": "grammar", "description": "x" }"#,
        )
        .unwrap();

        let opts = ScanOptions {
            builtin_root: Some(builtin.clone()),
            user_root: Some(user.clone()),
            registry: Value::Null,
            disabled: std::collections::HashSet::new(),
        };
        let res = scan(&opts);
        // too-new 被跳过；kr-grammar 用户根胜
        assert_eq!(res.sets.len(), 1);
        assert_eq!(res.sets[0].set_id, "kr-grammar");
        assert_eq!(res.sets[0].origin, SetOrigin::User);
        assert_eq!(res.sets[0].db_path, user_dir.join("knowledge.db"));
        // 用户包只有 1 条 → 版本相同用户覆盖内置
        assert_eq!(res.sets[0].counts.total, 1);
        assert!(
            res.warnings.iter().any(|w| w.contains("too-new")),
            "warnings={:?}",
            res.warnings
        );

        fs::remove_dir_all(&builtin).ok();
        fs::remove_dir_all(&user).ok();
    }

    #[test]
    fn higher_version_builtin_beats_lower_user() {
        let builtin = temp_root("builtin_high");
        let user = temp_root("user_low");
        write_minimal_package(&builtin, "kr-grammar", "2.0.0", &[("A", "I")]);
        write_minimal_package(&user, "kr-grammar", "1.5.0", &[("B", "II")]);

        let opts = ScanOptions {
            builtin_root: Some(builtin.clone()),
            user_root: Some(user.clone()),
            registry: Value::Null,
            disabled: std::collections::HashSet::new(),
        };
        let res = scan(&opts);
        assert_eq!(res.sets.len(), 1);
        assert_eq!(res.sets[0].origin, SetOrigin::Builtin);
        assert_eq!(res.sets[0].version, "2.0.0");

        fs::remove_dir_all(&builtin).ok();
        fs::remove_dir_all(&user).ok();
    }

    #[test]
    fn scan_with_missing_or_absent_roots_is_silent() {
        // 内置包删除后：内置根可能整体不存在 → 必须优雅跳过，不 panic、不产生假告警
        let missing = std::env::temp_dir().join(format!(
            "knoasis_missing_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&missing); // 确保不存在

        let opts = ScanOptions {
            builtin_root: Some(missing.clone()),
            user_root: None,
            registry: Value::Null,
            disabled: std::collections::HashSet::new(),
        };
        let res = scan(&opts);
        assert!(res.sets.is_empty(), "缺失根不应产出学科包");
        assert!(
            res.warnings.is_empty(),
            "缺失根不应产生假告警: {:?}",
            res.warnings
        );

        // 两个根都缺省（None）：同样安静返回空
        let res2 = scan(&ScanOptions::default());
        assert!(res2.sets.is_empty());
        assert!(res2.warnings.is_empty(), "缺省根不应产生告警: {:?}", res2.warnings);
    }

    #[test]
    fn helper_dto_shape_reference() {
        let _ = LevelOpt {
            code: "I".into(),
            label: "TOPIK I".into(),
            rank: 1,
        };
        let _ = TypeOpt {
            value: "grammar".into(),
            label: "语法".into(),
        };
    }
}
