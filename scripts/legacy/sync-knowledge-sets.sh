#!/usr/bin/env bash
# Knoasis · 同步 knowledge 学科包到应用数据目录
#
# 用途（docs/Knoasis-学科数据组织与第三方接入方案.md §3.4/§4.1/§5.7）：
#   把仓库 src-tauri/resources/knowledge/*.knowledgeset 分发到
#   $APPDATA/com.william.knoasis/knowledge/ 用户根，让 dev 环境即时读到新包。
#   第三方安装包也可直接拷入同一用户根（发现机制自动扫描）。
#
# 与旧 sync-grammar-db.sh 的关系（M4 前新旧并存）：
#   - 本脚本只分发新 knowledge 包，不动旧 grammar.db；
#   - 旧 UI 双轨期内仍由 sync-grammar-db.sh 提供旧 grammar.db（两个脚本可并行执行）。
#
# 用法：
#   scripts/sync-knowledge-sets.sh          # 默认源 src-tauri/resources/knowledge
#   KNASIS_KSET_SOURCE=/path scripts/sync-knowledge-sets.sh   # 指定源根
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SOURCE_ROOT="${KNASIS_KSET_SOURCE:-$REPO_ROOT/src-tauri/resources/knowledge}"

if [[ "$(uname)" == "Darwin" ]]; then
  APPDATA_DIR="$HOME/Library/Application Support"
else
  APPDATA_DIR="${APPDATA:-$HOME/.local/share}"
fi
DEST_ROOT="$APPDATA_DIR/com.william.knoasis/knowledge"

if [[ ! -d "$SOURCE_ROOT" ]]; then
  echo "错误：源 knowledge 根不存在：$SOURCE_ROOT" >&2
  exit 1
fi

mkdir -p "$DEST_ROOT"
count=0
for pkg in "$SOURCE_ROOT"/*.knowledgeset; do
  [[ -d "$pkg" ]] || continue
  name="$(basename "$pkg")"
  # 整目录覆盖同步（先删旧再拷贝，避免残留文件）
  rm -rf "$DEST_ROOT/$name"
  cp -R "$pkg" "$DEST_ROOT/$name"
  echo "✓ 已同步 → $DEST_ROOT/$name"
  count=$((count + 1))
done

if [[ "$count" -eq 0 ]]; then
  echo "提示：$SOURCE_ROOT 下没有 .knowledgeset 目录（请先运行 migrate-grammar-to-kset.py）" >&2
fi
echo "完成：同步 $count 个学科包到 $DEST_ROOT"
