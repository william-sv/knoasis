# legacy —— 已归档的学科包生产脚本

**状态：已退役（2026-09-11）。请勿在新流程中使用。**

这些脚本是学科包（`.knowledgeset`）生产能力的**第一代实现**（Python / Shell）。
该能力现已由独立 Rust 库承接：

> **`knoasis-studio/crates/knowledgeset-core`** —— 生产端核心库（不依赖 Tauri，可被 CLI / 第三方复用）
> 使用入口：Knoasis Studio 应用（导入源 JSON → 编辑 → 校验 → 导出 → 部署到用户知识根）

## 归档原因

| 问题 | 说明 |
|---|---|
| 实现分裂 | 同一套规则（字段映射 / 引号规范化 / summary 截断 / uid 派生 / DDL / 产物自检）在 Python 脚本与 Studio Rust 版中重复维护 |
| 无法复用 | 脚本与固定数据形态耦合，每接入一种新源数据就要再写一个脚本 |
| 缺少界面与校验闭环 | 改数据必须「改源 JSON → 跑脚本 → 手动同步」，且没有可视化编辑与契约校验 |

Studio 的 Rust 实现已完成端到端验证（真实 167 条数据全链路 + 42 项测试），
故将生产实现统一到 `knowledgeset-core`。

## 各脚本的原用途与替代

| 文件 | 原用途 | 现替代 |
|---|---|---|
| `import-en-v3-to-kset.py` | 英语语法源 JSON → `en-grammar.knowledgeset`（字段映射 + 引号规范化 + summary + DDL + 自检） | Studio「导入向导」+ `knowledgeset-core::importer/exporter` |
| `migrate-grammar-to-kset.py` | 旧 `grammar.db` → `kr-grammar.knowledgeset`（一次性迁移，642 条） | 迁移已完成；如需重跑可参考本文件，或改为「导出 → 导入向导」 |
| `sync-knowledge-sets.sh` | 把仓库内学科包同步到用户知识根 | Knoasis「管理学科包 → 导入学科包」（或 Studio 的「部署」按钮） |
| `sync-grammar-db.sh`（仍在 `scripts/`） | 旧 `grammar.db` 同步链路（与学科包无关） | 属 M4 退役范围，另行处理 |

## 保留策略

仅作历史记录与审计用途保留（git 历史亦可回溯）。若确认无需再参考，可直接删除本目录。
