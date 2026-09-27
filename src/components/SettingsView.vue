<script setup>
// 设置（独立整页）：主题外观 / 数据源状态 / 关于
// 主题三选一复用 ui.setThemePref；数据源取自 knowledgeSets store；返回按钮回到浏览态
import { computed } from "vue";
import { useUi } from "../stores/ui.js";
import { useKnowledgeSets } from "../stores/knowledgeSets.js";
import { knowledge } from "../lib/ipc.js";
// 应用版本 / 学科包 schema 版本直接读取仓库配置，避免写死
import appConfig from "../../src-tauri/tauri.conf.json";
import disciplineCatalog from "../../disciplines/catalog.json";

const ui = useUi();
const ks = useKnowledgeSets();

// 关于页展示：应用版本取自 tauri.conf.json 的 version；学科包 schema 版本取自 disciplines/catalog.json 的 version
const appVersion = computed(() => appConfig.version || "—");
const schemaVersion = computed(() => (disciplineCatalog.version ?? "—"));

const themeChoices = [
  { value: "system", label: "跟随系统", desc: "随系统深色 / 浅色外观自动切换" },
  { value: "light", label: "浅色", desc: "始终使用浅色外观" },
  { value: "dark", label: "深色", desc: "始终使用深色外观" },
];

function pickTheme(pref) {
  ui.setThemePref(pref);
}

// 用户知识根（学科包安装目录）来自 knowledge_reload 的 user_root
const userRoot = computed(() => (ks.dbInfo && ks.dbInfo.user_root) || "");
const dataSourceText = computed(() => {
  if (ks.loading) return "正在加载学科包数据…";
  if (ks.source === "error") return "学科包加载失败 · 可前往「学科管理」重试";
  if (ks.source !== "ipc") return "浏览器预览 · 未加载学科包";
  return userRoot.value || "—";
});

async function openRoot() {
  if (!userRoot.value) return;
  try {
    await knowledge.openDir(userRoot.value);
  } catch (e) {
    ui.showToast(`无法打开目录：${e?.message || e}`, 2600);
  }
}
</script>

<template>
  <section class="settings">
    <div class="settings-inner">
      <header class="settings-head">
        <button class="back-btn" title="返回浏览" @click="ui.gotoBrowse()">
          <span class="back-arrow">←</span>
          <span>返回</span>
        </button>
        <h1 class="settings-title">软件设置</h1>
      </header>

      <!-- 主题外观 -->
      <section class="card">
        <div class="card-label">主题外观</div>
        <div class="theme-choices">
          <button
            v-for="t in themeChoices"
            :key="t.value"
            class="theme-option"
            :class="{ 'is-active': ui.themePref === t.value }"
            @click="pickTheme(t.value)"
          >
            <div class="theme-option-main">
              <div class="theme-option-title">{{ t.label }}</div>
              <div class="theme-option-desc">{{ t.desc }}</div>
            </div>
            <span v-if="ui.themePref === t.value" class="check">✓</span>
          </button>
        </div>
      </section>

      <!-- 数据源 -->
      <section class="card">
        <div class="card-label">数据源</div>
        <div class="kv-list">
          <div class="kv">
            <span class="kv-key">学科包</span>
            <span class="kv-val">{{ ks.sets.length }} 个</span>
          </div>
          <div class="kv">
            <span class="kv-key">总条目数</span>
            <span class="kv-val">{{ ks.entries.length }} 条</span>
          </div>
          <div class="kv">
            <span class="kv-key">知识根</span>
            <span class="kv-val kv-val--path" :title="dataSourceText">{{ dataSourceText }}</span>
            <button v-if="userRoot" class="link-btn" @click="openRoot">打开目录</button>
          </div>
        </div>
      </section>

      <!-- 关于 -->
      <section class="card">
        <div class="card-label">关于</div>
        <div class="kv-list">
          <div class="kv">
            <span class="kv-key">应用</span>
            <span class="kv-val">Knoasis v{{ appVersion }}</span>
          </div>
          <div class="kv">
            <span class="kv-key">学科包 schema</span>
            <span class="kv-val">支持版本 {{ schemaVersion }}</span>
          </div>
        </div>
      </section>
    </div>
  </section>
</template>

<style scoped>
.settings {
  flex: 1;
  min-height: 0;
  overflow: auto;
  background: var(--bg);
}

.settings-inner {
  max-width: 760px;
  margin: 0 auto;
  padding: 28px 32px 48px;
}

/* 头部 + 返回 */
.settings-head {
  display: flex;
  align-items: center;
  gap: 14px;
  margin-bottom: 20px;
}
.back-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 30px;
  padding: 0 12px 0 10px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--panel);
  color: var(--text-muted);
  font-size: 12.5px;
  transition: background 0.12s ease, color 0.12s ease;
}
.back-btn:hover {
  background: var(--panel-hover);
  color: var(--text);
}
.back-arrow {
  font-size: 14px;
  line-height: 1;
}
.settings-title {
  margin: 0;
  font-size: 18px;
  font-weight: 700;
  color: var(--text);
}

/* 卡片 */
.card {
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  padding: 16px 18px;
}
.card + .card {
  margin-top: 16px;
}
.card-label {
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.4px;
  color: var(--text-faint);
  margin-bottom: 12px;
}

/* 主题选项 */
.theme-choices {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.theme-option {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  padding: 12px 14px;
  text-align: left;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--panel);
  transition: background 0.12s ease, border-color 0.12s ease;
}
.theme-option:hover {
  background: var(--panel-hover);
}
.theme-option.is-active {
  border-color: rgba(var(--accent-rgb), 0.4);
  background: rgba(var(--accent-rgb), 0.08);
}
.theme-option-main {
  flex: 1;
  min-width: 0;
}
.theme-option-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--text);
}
.theme-option-desc {
  margin-top: 3px;
  font-size: 11.5px;
  color: var(--text-faint);
}
.check {
  color: var(--accent);
  font-weight: 700;
  font-size: 13px;
}

/* 键值列表 */
.kv-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.kv {
  display: flex;
  align-items: baseline;
  gap: 12px;
}
.kv-key {
  flex: none;
  width: 96px;
  font-size: 12px;
  color: var(--text-faint);
}
.kv-val {
  flex: 1;
  min-width: 0;
  font-size: 12.5px;
  color: var(--text);
  word-break: break-word;
}
.kv-val--path {
  font-family: var(--font-mono);
  font-size: 11.5px;
  color: var(--text-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.link-btn {
  flex: none;
  height: 24px;
  padding: 0 9px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--panel);
  color: var(--text-muted);
  font-size: 11.5px;
  transition: background 0.12s ease, color 0.12s ease;
}
.link-btn:hover {
  background: var(--panel-hover);
  color: var(--text);
}
</style>
