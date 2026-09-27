<script setup>
// 首页（独立整页）：品牌标题 + 2×2 功能卡片入口
// - 学科下载：打开「学科包下载」弹窗
// - 学科管理：打开学科包管理弹窗
// - 软件设置：进入设置整页
// - 指南：在浏览器打开外部文档（未配置时提示）
import { computed } from "vue";
import { useUi } from "../stores/ui.js";
import { useKnowledgeSets } from "../stores/knowledgeSets.js";
import { openExternal, USER_GUIDE_URL } from "../lib/links.js";
import logoAsset from "../assets/logo.png";
import downloadAsset from "../assets/icons/download.png";
import docsetsAsset from "../assets/icons/docsets.png";
import settingAsset from "../assets/icons/setting.png";
import guideAsset from "../assets/icons/user-guide.png";

const ui = useUi();
const ks = useKnowledgeSets();

// 空态引导：一个学科包都没有（首次启动 / 全部删除）时给出明确导入入口
const showOnboarding = computed(() => !ks.loading && ks.source !== "error" && ks.sets.length === 0);

// 加载失败：持久可见的错误态（给出原因与重试入口，避免首页静默空白）
const loadFailed = computed(() => !ks.loading && ks.source === "error");
const loadErrorText = computed(() => ks.loadError || "未知错误");

async function onRetry() {
  await ks.reload();
}

function onDownload() {
  ui.openDownload();
}

function onManage() {
  ui.openSetsManager();
}

function onSettings() {
  ui.gotoSettings();
}

async function onUserGuide() {
  if (!USER_GUIDE_URL) {
    ui.showToast("指南链接未配置");
    return;
  }
  const ok = await openExternal(USER_GUIDE_URL);
  if (!ok) ui.showToast("无法打开指南（请在桌面应用中打开）");
}
</script>

<template>
  <section class="home">
    <div class="home-inner">
      <header class="home-head">
        <img class="home-logo" :src="logoAsset" alt="Knoasis" />
        <h1 class="home-title">Knoasis</h1>
        <p class="home-sub">诺西斯 · 你的知识绿洲，让每一个知识点都有归处</p>
      </header>

      <!-- 加载失败：持久错误态（原因 + 重试 / 管理入口） -->
      <div v-if="loadFailed" class="home-error">
        <div class="error-title">学科数据加载失败</div>
        <div class="error-hint">{{ loadErrorText }}</div>
        <div class="error-actions">
          <button class="btn btn--primary" @click="onRetry">重新加载</button>
          <button class="btn" @click="onManage">管理学科包…</button>
        </div>
      </div>

      <!-- 空态引导：尚无任何学科包时，突出「导入」入口 -->
      <div v-if="showOnboarding" class="home-onboarding">
        <div class="onboarding-title">还没有任何学科包</div>
        <div class="onboarding-hint">
          导入一个 <code>*.knowledgeset</code> 学科包即可开始浏览知识点；也可以先看看有哪些学科包可以下载。
        </div>
        <div class="onboarding-actions">
          <button class="btn btn--primary" @click="onManage">导入学科包…</button>
          <button class="btn" @click="onDownload">浏览可下载学科包</button>
        </div>
      </div>

      <div class="home-grid">
        <button class="home-card" @click="onDownload">
          <img class="card-icon" :src="downloadAsset" alt="" />
          <div class="card-body">
            <div class="card-title">学科下载</div>
            <div class="card-desc">浏览并下载全新的学科知识包</div>
          </div>
        </button>

        <button class="home-card" @click="onManage">
          <img class="card-icon" :src="docsetsAsset" alt="" />
          <div class="card-body">
            <div class="card-title">学科管理</div>
            <div class="card-desc">导入、删除或启用停用本地学科包</div>
          </div>
        </button>

        <button class="home-card" @click="onSettings">
          <img class="card-icon" :src="settingAsset" alt="" />
          <div class="card-body">
            <div class="card-title">软件设置</div>
            <div class="card-desc">外观主题、数据源状态与版本信息</div>
          </div>
        </button>

        <button class="home-card" @click="onUserGuide">
          <img class="card-icon" :src="guideAsset" alt="" />
          <div class="card-body">
            <div class="card-title">指南</div>
            <div class="card-desc">查看使用说明与常见问题文档</div>
          </div>
        </button>
      </div>
    </div>
  </section>
</template>

<style scoped>
.home {
  flex: 1;
  min-height: 0;
  overflow: auto;
  background: var(--bg);
}

.home-inner {
  max-width: 880px;
  margin: 0 auto;
  padding: 56px 32px 48px;
}

/* 标题区 */
.home-head {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  gap: 6px;
  margin-bottom: 34px;
}
.home-logo {
  height: 44px;
  width: auto;
  margin-bottom: 4px;
}
.home-title {
  margin: 0;
  font-size: 26px;
  font-weight: 750;
  letter-spacing: 0.4px;
  color: var(--text);
}
.home-sub {
  margin: 0;
  font-size: 13px;
  color: var(--text-muted);
}

/* 空态引导 */
.home-onboarding {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  gap: 8px;
  margin-bottom: 22px;
  padding: 20px 24px;
  border: 1px dashed rgba(var(--accent-rgb), 0.45);
  border-radius: var(--radius);
  background: rgba(var(--accent-rgb), 0.06);
}
.onboarding-title {
  font-size: 14px;
  font-weight: 650;
  color: var(--text);
}
.onboarding-hint {
  font-size: 12px;
  line-height: 1.7;
  color: var(--text-muted);
  max-width: 460px;
}
.onboarding-hint code {
  font-family: var(--font-mono);
  font-size: 0.92em;
  background: var(--code-bg);
  border: 1px solid var(--border-soft);
  border-radius: 4px;
  padding: 1px 5px;
}
.onboarding-actions {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-top: 4px;
}

/* 加载失败持久错误态 */
.home-error {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  gap: 8px;
  margin-bottom: 22px;
  padding: 18px 24px;
  border: 1px solid rgba(229, 72, 77, 0.45);
  border-radius: var(--radius);
  background: rgba(229, 72, 77, 0.08);
}
.error-title {
  font-size: 14px;
  font-weight: 650;
  color: #e5484d;
}
.error-hint {
  font-size: 12px;
  line-height: 1.7;
  color: var(--text-muted);
  max-width: 480px;
  word-break: break-word;
}
.error-actions {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-top: 4px;
}

/* 2×2 卡片网格 */
.home-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 16px;
}

.home-card {
  display: flex;
  align-items: flex-start;
  gap: 14px;
  padding: 20px 20px;
  text-align: left;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--panel);
  transition: background 0.14s ease, border-color 0.14s ease, transform 0.14s ease,
    box-shadow 0.14s ease;
}
.home-card:hover {
  background: var(--panel-hover);
  border-color: rgba(var(--accent-rgb), 0.34);
  box-shadow: var(--shadow-1);
  transform: translateY(-1px);
}
.home-card:active {
  transform: translateY(0);
}
.card-icon {
  width: 44px;
  height: 44px;
  object-fit: contain;
  flex: none;
}
.card-body {
  min-width: 0;
}
.card-title {
  font-size: 14px;
  font-weight: 650;
  color: var(--text);
}
.card-desc {
  margin-top: 4px;
  font-size: 12px;
  line-height: 1.6;
  color: var(--text-muted);
}
</style>
