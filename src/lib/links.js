// Knoasis · 外部链接
//
// 统一收口「打开外部文档」的能力：Tauri 内走 @tauri-apps/plugin-opener 的 openUrl；
// 纯浏览器预览或链接未配置时安全降级（返回 false、不抛异常），由调用方决定提示文案。

import { isTauri } from "./ipc.js";

/** 指南外部文档地址（应用内「指南」按钮跳转；文档目录：guides/README.md） */
export const USER_GUIDE_URL = "https://github.com/william-sv/knoasis/blob/main/guides/README.md";

/**
 * 在系统默认浏览器中打开外部链接。
 * @param {string} url 目标地址
 * @returns {Promise<boolean>} 是否成功发起打开（非 Tauri / 空值 / 异常均返回 false）
 */
export async function openExternal(url) {
  if (!url) return false;
  if (!isTauri()) return false;
  try {
    // 动态引入：未安装 opener 插件时不阻塞页面加载
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(url);
    return true;
  } catch {
    return false;
  }
}
