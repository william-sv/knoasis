// Knoasis · knowledge 学科包只读模块（v1 契约，见 docs/Knoasis-学科数据组织与第三方接入方案.md §3/§5）
//
// 职责：
//  1. discovery —— 扫描两个知识根（随包 $RESOURCE/knowledge + 用户 $APPDATA/.../knowledge）
//     遍历 `*.knowledgeset/meta.json`，冲突规则：同 set_id 版本高者胜、同版本用户根胜
//  2. meta —— meta.json 解析与校验 + template 解析（包内 > 内置 registry > 通用默认）
//  3. reader —— 按包只读打开 knowledge.db；list_entries / get_entry / resolve_assets
//  4. uid —— `{set_id}:{hex(sha1(headword))[0..12]}` 派生与按前缀路由
//  5. commands —— knowledge_* IPC 命令薄壳
//
// 数据模型约定（冻结）：entries / entry_detail 固定语义表名；详情 = view-ready JSON 单列；
// 前端渲染器（DetailSections）按 template.sections 驱动，字段键以 adapter.js toDetailView 输出为准。

pub mod commands;
pub mod discovery;
pub mod manager;
pub mod meta;
pub mod reader;
pub mod uid;

pub use commands::*;

use serde::Serialize;
use std::collections::BTreeMap;

/// 当前支持的 meta.schema_version（学科包内容契约版本）
pub const SUPPORTED_SCHEMA_VERSION: i64 = 1;

/// 学科包来源根（冲突/展示用）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum SetOrigin {
    #[serde(rename = "builtin")]
    Builtin,
    #[serde(rename = "user")]
    User,
}

impl SetOrigin {
    pub fn as_str(&self) -> &'static str {
        match self {
            SetOrigin::Builtin => "builtin",
            SetOrigin::User => "user",
        }
    }
}

/// 等级选项（meta.levels.values / 前端筛选下拉）
#[derive(Debug, Clone, Serialize)]
pub struct LevelOpt {
    pub code: String,
    pub label: String,
    pub rank: u32,
}

/// 类型选项（meta.types）
#[derive(Debug, Clone, Serialize)]
pub struct TypeOpt {
    pub value: String,
    pub label: String,
}

/// 学科计数（list_sets 直接展示 / by_level 含 '' 未分级桶）
#[derive(Debug, Clone, Default, Serialize)]
pub struct SubjectCounts {
    pub total: usize,
    pub by_level: BTreeMap<String, usize>,
}

/// 学科包元信息（发现机制产出；含从 DB 计算的 counts）
#[derive(Debug, Clone, Serialize)]
pub struct KnowledgeSetMeta {
    /// 目录名主体 = meta.id = uid 前缀 = userdata 学科键
    pub set_id: String,
    pub name: String,
    pub version: String,
    pub schema_version: i64,
    pub language: String,
    pub kind: String,
    pub color: String,
    pub description: String,
    pub level_system: String,
    pub level_system_label: String,
    pub levels: Vec<LevelOpt>,
    pub types: Vec<TypeOpt>,
    pub template: serde_json::Value,
    pub counts: SubjectCounts,
    pub origin: SetOrigin,
    /// 包内 knowledge.db 绝对路径
    #[serde(skip_serializing)]
    pub db_path: std::path::PathBuf,
    /// 包内 assets/ 目录（可选；无则为 None）
    #[serde(skip_serializing)]
    pub assets_dir: Option<std::path::PathBuf>,
}

/// knowledge_list_sets 返回的 IPC 形状（与既有 grammar SubjectMeta 键对齐，toSetShape 零改动）
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

impl From<&KnowledgeSetMeta> for SubjectMeta {
    fn from(m: &KnowledgeSetMeta) -> Self {
        SubjectMeta {
            id: m.set_id.clone(),
            name: m.name.clone(),
            color: m.color.clone(),
            discipline: m.set_id.clone(),
            level_system: m.level_system.clone(),
            level_system_label: m.level_system_label.clone(),
            levels: m.levels.clone(),
            types: m.types.clone(),
            template: m.template.clone(),
            counts: m.counts.clone(),
            description: m.description.clone(),
        }
    }
}

/// 列表条目（EntryItem 形状与既有 grammar 对齐）
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

#[derive(Debug, Clone, Default, Serialize)]
pub struct ListEntriesResult {
    pub total: usize,
    pub items: Vec<EntryItem>,
}

/// 已解析到本地绝对路径的包内资源（assetUrl(path) 渲染 <img>）
#[derive(Debug, Clone, Serialize)]
pub struct AssetImage {
    /// 包内相对路径（content JSON / image_hidden 主键值）
    pub rel: String,
    /// 已解析本地绝对路径
    pub path: String,
}

/// 详情载荷：条目标头 + view-ready content JSON + 可见图（按 userdata image_hidden 过滤）
#[derive(Debug, Clone, Serialize)]
pub struct EntryPayload {
    pub uid: String,
    pub set_id: String,
    pub headword: String,
    pub category: String,
    pub level_code: String,
    pub level_label: String,
    pub tags: Vec<String>,
    pub summary: String,
    pub content: serde_json::Value,
    pub images: Vec<AssetImage>,
}

/// knowledge_reload 返回
#[derive(Debug, Clone, Serialize)]
pub struct ReloadInfo {
    pub sets: Vec<SubjectMeta>,
    pub entries_total: usize,
    /// 用户知识根（学科包安装目录；未创建时为空串），供设置页展示与「打开目录」
    pub user_root: String,
}

// ---------------------------------------------------------------------------
// 模块级基础单测：uid/discovery/schema_version 集成冒烟
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::discovery::{self, ScanOptions};
    use crate::knowledge::reader::{self, EntryFilter};
    use rusqlite::{params, Connection};
    use std::fs;

    fn temp_root(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "knoasis_knowledge_mod_{}_{tag}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_pkg(root: &std::path::Path, set_id: &str, version: &str) {
        let dir = root.join(format!("{set_id}.knowledgeset"));
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("meta.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "schema_version": 1,
                "id": set_id,
                "name": set_id,
                "version": version,
                "language": "ko",
                "kind": "grammar",
                "description": "模块冒烟",
                "levels": {
                    "system": "topik",
                    "label": "TOPIK",
                    "values": [
                        { "code": "I", "label": "TOPIK I", "rank": 1 },
                        { "code": "II", "label": "TOPIK II", "rank": 2 }
                    ]
                },
                "types": [ { "value": "grammar", "label": "语法" } ]
            }))
            .unwrap(),
        )
        .unwrap();
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
        let rows = [("N마저", "II", "s"), ("-고", "I", "s")];
        for (i, (hw, lv, sum)) in rows.iter().enumerate() {
            let uid_val = uid::derive(set_id, hw);
            conn.execute(
                "INSERT INTO entries(id, uid, headword, category, level, tags, summary) \
                 VALUES(?1,?2,?3,'类',?4,'',?5)",
                params![(i + 1) as i64, uid_val, hw, lv, sum],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO entry_detail(entry_id, content) VALUES(?1,'{\"fields\":{}}')",
                params![(i + 1) as i64],
            )
            .unwrap();
        }
        drop(conn);
    }

    #[test]
    fn module_scan_list_get_integration() {
        let root = temp_root("integ");
        write_pkg(&root, "kr-grammar", "1.0.0");
        let res = discovery::scan(&ScanOptions {
            builtin_root: Some(root.clone()),
            user_root: None,
            registry: serde_json::Value::Null,
            disabled: std::collections::HashSet::new(),
        });
        assert_eq!(res.sets.len(), 1);
        let meta = &res.sets[0];
        assert_eq!(meta.counts.total, 2);
        // template 缺包内/registry → 默认模板可用
        assert!(meta.template.get("sections").is_some());

        // list entries
        let listed = reader::list_entries(meta, &EntryFilter { limit: 50, ..Default::default() })
            .unwrap();
        assert_eq!(listed.total, 2);
        assert_eq!(listed.items.len(), 2);

        // get entry by uid
        let uid_val = uid::derive("kr-grammar", "N마저");
        let payload = reader::get_entry(meta, &uid_val, &std::collections::HashSet::new())
            .unwrap();
        assert_eq!(payload.headword, "N마저");
        assert_eq!(payload.level_label, "TOPIK II");
        assert_eq!(payload.images.len(), 0);

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn subject_meta_maps_from_knowledge_set() {
        let root = temp_root("subj");
        write_pkg(&root, "kr-grammar", "1.0.0");
        let res = discovery::scan(&ScanOptions {
            builtin_root: Some(root.clone()),
            user_root: None,
            registry: serde_json::Value::Null,
            disabled: std::collections::HashSet::new(),
        });
        let sm = SubjectMeta::from(&res.sets[0]);
        assert_eq!(sm.id, "kr-grammar");
        assert_eq!(sm.discipline, "kr-grammar");
        assert_eq!(sm.levels.len(), 2);
        fs::remove_dir_all(&root).ok();
    }
}
