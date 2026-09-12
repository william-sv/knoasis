#!/usr/bin/env bash
# Knoasis · 同步 grammar.db 到应用数据目录
# 用途：数据刷新无需重编译。运行 npm run tauri dev 前可先执行本脚本，
# 也可用 export KNASIS_GRAMMAR_DB=… 让运行时直接指向工作区 db（优先级最高）。
#
# 用法：
#   scripts/sync-grammar-db.sh                 # 把 data/grammar/grammar.db 拷到应用数据目录
#   KNASIS_GRAMMAR_DB_SOURCE=/path/db.sh sync   # 指定源文件
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SOURCE="${KNASIS_GRAMMAR_DB_SOURCE:-$REPO_ROOT/data/grammar/grammar.db}"
TEMPLATE_SOURCE="${KNASIS_TEMPLATE_REGISTRY_SOURCE:-$REPO_ROOT/src-tauri/resources/grammar/template_registry.json}"

if [[ "$(uname)" == "Darwin" ]]; then
  APPDATA_DIR="$HOME/Library/Application Support"
else
  APPDATA_DIR="${APPDATA:-$HOME/.local/share}"
fi
DEST_DIR="$APPDATA_DIR/com.william.knoasis/grammar"

if [[ ! -f "$SOURCE" ]]; then
  echo "错误：源 grammar.db 不存在：$SOURCE" >&2
  exit 1
fi

mkdir -p "$DEST_DIR"
cp "$SOURCE" "$DEST_DIR/grammar.db"
echo "✓ grammar.db 已同步 → $DEST_DIR/grammar.db"

if [[ -f "$TEMPLATE_SOURCE" ]]; then
  cp "$TEMPLATE_SOURCE" "$DEST_DIR/template_registry.json"
  echo "✓ template_registry.json 已同步 → $DEST_DIR/template_registry.json"
fi

# 英语语法图解：与 grammar.db 同根 en_images/（英语接入设计 §4.6）
IMAGE_SOURCE="${KNASIS_GRAMMAR_IMAGES_SOURCE:-$REPO_ROOT/src-tauri/resources/grammar/en_images}"
if [[ -d "$IMAGE_SOURCE" ]]; then
  mkdir -p "$DEST_DIR/en_images"
  cp -R "$IMAGE_SOURCE/." "$DEST_DIR/en_images/"
  echo "✓ en_images 已同步 → $DEST_DIR/en_images/"
else
  echo "提示：未找到 en_images 目录（$IMAGE_SOURCE），跳过图解同步" >&2
fi
