// Knoasis · 本地资源 URL（Tauri asset 协议）
//
// 学科包内资源（如 *.knowledgeset/assets/ 下图片）离线随包，通过 Tauri asset 协议暴露给 <img>。
//  - Tauri 内：convertFileSrc(path) → asset://… 让 webview 直读本地资源（懒加载、webview 图片缓存）
//  - 非 Tauri（浏览器预览）：返回空串，调用方不渲染（浏览器本就是空态）
// asset scope 见 src-tauri/tauri.conf.json app.security.assetProtocol
//   —— 实际配置：["$APPDATA/knowledge/**"]（仅用户知识根）。
// 注意 $APPDATA 在 Tauri v2 已展开为 app_data_dir()，其值 **本身即含 bundle 标识符**：
//   app_data_dir() === {dataDir}/{bundleIdentifier}/，本机即
//   ~/Library/Application Support/com.william.knoasis/
// 因此正确的放行路径是 $APPDATA/knowledge/**（等价于 …/com.william.knoasis/knowledge/**）。
// 切勿再写成旧的双标识符形式 $APPDATA/com.william.knoasis/knowledge/**
//   —— 那会被解析成 …/com.william.knoasis/com.william.knoasis/knowledge/**，是无效路径。

import { convertFileSrc, isTauri } from "@tauri-apps/api/core";

/** 本地绝对路径 → <img> 可用的 asset URL；空/非 Tauri 返回 "" */
export function assetUrl(p) {
  return p && isTauri() ? convertFileSrc(p) : "";
}
