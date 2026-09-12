// Knoasis · 学科包 meta.json 解析与校验 + 详情模板解析
//
// 契约见 docs/Knoasis-学科数据组织与第三方接入方案.md §5.2 / §5.4。
// 必填：schema_version/id/name/version/language/kind/description；
// schema_version > 当前支持则拒（需升级 App）。levels/types/template 结构校验后进 KnowledgeSetMeta。
// template 来源优先级：包内 template.json > 内置 registry 同名段（旧包兼容）> 通用默认。

use crate::knowledge::{LevelOpt, SUPPORTED_SCHEMA_VERSION};
use serde_json::Value;
use std::path::Path;

/// 解析结果（不含模板/template 合并由 resolve_template 完成；counts/db 路径由 discovery 填）
#[derive(Debug, Clone)]
pub struct ParsedMeta {
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
    pub types: Vec<crate::knowledge::TypeOpt>,
}

fn type_str<'a>(v: &'a Value, key: &str) -> Result<&'a str, String> {
    v.get(key)
        .and_then(|x| x.as_str())
        .ok_or_else(|| format!("meta.json 缺少 string 字段: {key}"))
}

fn type_i64(v: &Value, key: &str) -> Result<i64, String> {
    v.get(key)
        .and_then(|x| x.as_i64())
        .ok_or_else(|| format!("meta.json 缺少 int 字段: {key}"))
}

fn type_str_or<'a>(v: &'a Value, key: &str, default: &'a str) -> &'a str {
    v.get(key).and_then(|x| x.as_str()).unwrap_or(default)
}

/// 解析 meta.json 文本（JSON 对象）。任何必填缺失 / 类型错误 / 版本过高都返回 Err（调用方跳过包）。
pub fn parse_meta_json(text: &str) -> Result<ParsedMeta, String> {
    let root: Value = serde_json::from_str(text)
        .map_err(|e| format!("meta.json 不是合法 JSON: {e}"))?;
    let obj = root
        .as_object()
        .ok_or_else(|| "meta.json 顶层必须是 JSON 对象".to_string())?;
    if obj.is_empty() {
        return Err("meta.json 为空对象".to_string());
    }

    let schema_version = type_i64(&root, "schema_version")?;
    if schema_version > SUPPORTED_SCHEMA_VERSION {
        return Err(format!(
            "meta.json schema_version={schema_version} 高于当前支持 {SUPPORTED_SCHEMA_VERSION}：请升级应用"
        ));
    }

    let set_id = type_str(&root, "id")?.to_string();
    if !crate::knowledge::uid::is_valid_set_id(&set_id) {
        return Err(format!("meta.json id 非法（须 ^[a-z0-9]+(-[a-z0-9]+)*$）: {set_id}"));
    }

    let name = type_str(&root, "name")?.to_string();
    let version = type_str(&root, "version")?.to_string();
    let language = type_str(&root, "language")?.to_string();
    let kind = type_str(&root, "kind")?.to_string();
    let description = type_str(&root, "description")?.to_string();

    let color = type_str_or(&root, "color", "").to_string();

    // levels（可选）：{ system?, label?, values: [{code,label,rank?}] }
    let mut level_system = String::new();
    let mut level_system_label = String::new();
    let mut levels: Vec<LevelOpt> = Vec::new();
    if let Some(lv) = root.get("levels") {
        let lv = lv
            .as_object()
            .ok_or_else(|| "meta.json levels 必须是对象".to_string())?;
        level_system = lv
            .get("system")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        level_system_label = lv
            .get("label")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let values = lv
            .get("values")
            .and_then(|x| x.as_array())
            .ok_or_else(|| "meta.json levels.values 必须是数组".to_string())?;
        for (i, item) in values.iter().enumerate() {
            let code = item
                .get("code")
                .and_then(|x| x.as_str())
                .ok_or_else(|| format!("meta.json levels.values[{i}] 缺 string code"))?
                .to_string();
            if code.is_empty() {
                return Err(format!("meta.json levels.values[{i}] code 为空"));
            }
            let label = item
                .get("label")
                .and_then(|x| x.as_str())
                .unwrap_or(&code)
                .to_string();
            let rank = item
                .get("rank")
                .and_then(|x| x.as_u64())
                .unwrap_or((i as u64) + 1) as u32;
            levels.push(LevelOpt { code, label, rank });
        }
    }

    // types（可选）：[{value,label?}]
    let mut types: Vec<crate::knowledge::TypeOpt> = Vec::new();
    if let Some(ts) = root.get("types") {
        let ts = ts
            .as_array()
            .ok_or_else(|| "meta.json types 必须是数组".to_string())?;
        for (i, item) in ts.iter().enumerate() {
            let value = item
                .get("value")
                .and_then(|x| x.as_str())
                .ok_or_else(|| format!("meta.json types[{i}] 缺 string value"))?
                .to_string();
            if value.is_empty() {
                return Err(format!("meta.json types[{i}] value 为空"));
            }
            let label = item
                .get("label")
                .and_then(|x| x.as_str())
                .unwrap_or(&value)
                .to_string();
            types.push(crate::knowledge::TypeOpt { value, label });
        }
    }
    if types.is_empty() {
        // 缺省 = 单类型（kind 作语义分类）
        types.push(crate::knowledge::TypeOpt {
            value: kind.clone(),
            label: kind.clone(),
        });
    }

    Ok(ParsedMeta {
        set_id,
        name,
        version,
        schema_version,
        language,
        kind,
        color,
        description,
        level_system,
        level_system_label,
        levels,
        types,
    })
}

/// 通用默认模板（包内与 registry 均缺时兜底：fields + headword + summary 的可渲染最小节）
pub fn default_template(name: &str) -> Value {
    serde_json::json!({
        "label": name,
        "headwordLabel": "条目",
        "sections": [
            {
                "key": "headword",
                "label": "条目",
                "type": "fields",
                "fields": [
                    { "key": "headword", "label": "条目" },
                    { "key": "summary", "label": "摘要" }
                ]
            }
        ]
    })
}

fn read_json_file(p: &Path) -> Option<Value> {
    let text = std::fs::read_to_string(p).ok()?;
    serde_json::from_str(&text).ok()
}

/// 按 set_id 从 registry 取同名段（旧包 registry key 可能用下划线 kr_grammar；先原样再替换 '-'→'_'）
fn registry_segment(registry: &Value, set_id: &str) -> Option<Value> {
    if let Some(v) = registry.get(set_id) {
        if v.is_object() {
            return Some(v.clone());
        }
    }
    let legacy = set_id.replace('-', "_");
    if legacy != set_id {
        if let Some(v) = registry.get(&legacy) {
            if v.is_object() {
                return Some(v.clone());
            }
        }
    }
    None
}

/// 解析包内 template.json 或 registry 同名段或通用默认。
/// registry 为应用内置 template_registry 全文（可为 Value::Null）。
pub fn resolve_template(set_dir: &Path, set_id: &str, name: &str, registry: &Value) -> Value {
    // 1. 包内 template.json
    let pkg = set_dir.join("template.json");
    if let Some(v) = read_json_file(&pkg) {
        if v.is_object() {
            return v;
        }
    }
    // 2. 内置 registry 同名段（旧包兼容）
    if let Some(seg) = registry_segment(registry, set_id) {
        return seg;
    }
    // 3. 通用默认
    default_template(name)
}

/// 校验目录名 = {set_id}.knowledgeset（结构约束；不符视为非法包）。
pub fn dir_matches_set_id(dir_name: &str, set_id: &str) -> bool {
    dir_name == format!("{set_id}.knowledgeset")
}

// ---------------------------------------------------------------------------
// 单元测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_META: &str = r##"{
      "schema_version": 1,
      "id": "kr-grammar",
      "name": "韩语语法",
      "version": "1.0.0",
      "language": "ko",
      "kind": "grammar",
      "description": "韩语语法 · TOPIK I/II 分级",
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
    }"##;

    #[test]
    fn parse_valid_meta_ok() {
        let m = parse_meta_json(VALID_META).unwrap();
        assert_eq!(m.set_id, "kr-grammar");
        assert_eq!(m.schema_version, 1);
        assert_eq!(m.color, "#D97706");
        assert_eq!(m.level_system, "topik");
        assert_eq!(m.levels.len(), 2);
        assert_eq!(m.levels[0].label, "TOPIK I");
        assert_eq!(m.types.len(), 1);
        assert_eq!(m.types[0].value, "grammar");
    }

    #[test]
    fn reject_schema_version_too_high() {
        let meta = VALID_META.replace("\"schema_version\": 1", "\"schema_version\": 99");
        let err = parse_meta_json(&meta).unwrap_err();
        assert!(err.contains("高于当前支持"), "err={err}");
    }

    #[test]
    fn reject_bad_id_and_missing_fields() {
        let bad = VALID_META.replace("\"id\": \"kr-grammar\"", "\"id\": \"Kr_Grammar\"");
        assert!(parse_meta_json(&bad).is_err());

        let missing = VALID_META.replace("\"description\": \"韩语语法 · TOPIK I/II 分级\",", "");
        assert!(parse_meta_json(&missing).is_err());
    }

    #[test]
    fn template_fallback_chain() {
        // 临时目录：无 template.json → registry 同名段；registry 也缺 → 默认模板
        let dir = std::env::temp_dir().join(format!(
            "knoasis_meta_tmpl_{}_{}",
            std::process::id(),
            uuid4like()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let registry = serde_json::json!({ "kr_grammar": { "label": "旧段", "sections": [] } });

        // registry 用下划线旧 key
        let t = resolve_template(&dir, "kr-grammar", "韩语语法", &registry);
        assert_eq!(t["label"], "旧段");

        // registry 缺 → 默认模板
        let t2 = resolve_template(&dir, "unknown-set", "未知", &Value::Null);
        assert!(t2.get("sections").is_some());

        // 包内 template.json 优先
        std::fs::write(
            dir.join("template.json"),
            r#"{ "label": "包内模板", "sections": [] }"#,
        )
        .unwrap();
        let t3 = resolve_template(&dir, "kr-grammar", "韩语语法", &registry);
        assert_eq!(t3["label"], "包内模板");

        std::fs::remove_dir_all(&dir).ok();
    }

    fn uuid4like() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos().to_string())
            .unwrap_or_default()
    }
}
