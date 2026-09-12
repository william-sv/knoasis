// Knoasis · knowledge uid 派生 / 路由 / 校验
//
// 契约（v1）：
//   uid = "{set_id}:{hex(sha1(headword))[0..12]}"
//   set_id 即 uid 前缀 → 按冒号前段路由到学科包；学科包内 headword 撞车在构建/迁移期
//   用「确定性后缀追加到 headword」解决，不落自增 id 作外部键。
//   headword 变化才变化；重导入同 headword → 同 uid（收藏/笔记/删图以 uid 锚定）。

use sha1::{Digest, Sha1};

/// 派生 uid 的 hex 主体：hex(sha1(headword))[0..12]（sha1 输入 headword 原始 UTF-8）
///
/// 注：当前只读读取链路不调用（包内 uid 直接读取即可）；本函数与
/// scripts/migrate-grammar-to-kset.py 的 derive_uid 完全一致，作为 uid 派生算法的
/// 规范参照，被各模块单元测试与后续包生成工具使用。
#[allow(dead_code)]
pub fn uid_hex(headword: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(headword.as_bytes());
    let digest = hasher.finalize();
    let hex: String = digest.iter().map(|b| format!("{:02x}", b)).collect();
    hex[..12].to_string()
}

/// uid = "{set_id}:{hex12}"（同上：规范参照，供测试/包生成工具使用）
#[allow(dead_code)]
pub fn derive(set_id: &str, headword: &str) -> String {
    format!("{set_id}:{}", uid_hex(headword))
}

/// uid 前缀 → set_id（冒号前段；无冒号返回 None）
pub fn set_from_uid(uid: &str) -> Option<&str> {
    let (set_id, _rest) = uid.split_once(':')?;
    if set_id.is_empty() {
        return None;
    }
    Some(set_id)
}

/// 是否合法 set_id：小写 `^[a-z0-9]+(-[a-z0-9]+)*$`
pub fn is_valid_set_id(set_id: &str) -> bool {
    let mut parts = set_id.split('-');
    let Some(first) = parts.next() else {
        return false;
    };
    if !is_alnum(first) {
        return false;
    }
    parts.all(|p| !p.is_empty() && is_alnum(p))
}

fn is_alnum(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
}

/// 校验完整 uid 结构：合法 set_id + ':' + 至少 1 位 hex 后缀（撞车补偿后缀 -2 也允许）
pub fn is_valid_uid(uid: &str) -> bool {
    let Some((set_id, rest)) = uid.split_once(':') else {
        return false;
    };
    if !is_valid_set_id(set_id) || rest.is_empty() {
        return false;
    }
    // hex12 主体 + 可选 "-N" 补偿后缀；宽松校验后缀为小写 hex 或合法补偿串
    rest.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

// ---------------------------------------------------------------------------
// 单元测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_is_deterministic_and_formatted() {
        let a = derive("kr-grammar", "N마저");
        let b = derive("kr-grammar", "N마저");
        assert_eq!(a, b);
        assert!(a.starts_with("kr-grammar:"));
        // "kr-grammar:" 11 字符 + 12 hex
        assert_eq!(a.len(), "kr-grammar:".len() + 12);
        assert_eq!(a, format!("kr-grammar:{}", uid_hex("N마저")));

        let c = derive("kr-grammar", "아이");
        assert_ne!(a, c);
        // 不同 set_id 同 headword → 前缀不同
        let d = derive("en-grammar", "N마저");
        assert!(d.starts_with("en-grammar:"));
        assert_ne!(a, d);
    }

    #[test]
    fn set_from_uid_routes_by_prefix() {
        assert_eq!(set_from_uid("kr-grammar:1f2e3d4c5b6a"), Some("kr-grammar"));
        assert_eq!(set_from_uid("en-grammar:abcd1234ef56"), Some("en-grammar"));
        // 撞车补偿后缀原样保留在冒号后
        assert_eq!(set_from_uid("kr-grammar:1f2e3d4c5b6a-2"), Some("kr-grammar"));
        // 无冒号 / 空前缀
        assert_eq!(set_from_uid("noprefix"), None);
        assert_eq!(set_from_uid(":abc"), None);
        assert_eq!(set_from_uid(""), None);
    }

    #[test]
    fn set_id_validation() {
        assert!(is_valid_set_id("kr-grammar"));
        assert!(is_valid_set_id("en"));
        assert!(is_valid_set_id("math-theorem-2"));
        assert!(is_valid_set_id("a1-b2-c3"));
        assert!(!is_valid_set_id("Kr-Grammar")); // 大写非法
        assert!(!is_valid_set_id("-grammar"));
        assert!(!is_valid_set_id("kr--grammar"));
        assert!(!is_valid_set_id("kr_grammar")); // 下划线非法
        assert!(!is_valid_set_id(""));
        assert!(!is_valid_set_id("한국어"));
    }

    #[test]
    fn full_uid_validation() {
        assert!(is_valid_uid("kr-grammar:1f2e3d4c5b6a"));
        assert!(is_valid_uid("kr-grammar:1f2e3d4c5b6a-2"));
        assert!(!is_valid_uid("kr_grammar:1f2e3d4c5b6a")); // set 部分非法
        assert!(!is_valid_uid("kr-grammar:")); // 无后缀
        assert!(!is_valid_uid("noprefix"));
    }
}
