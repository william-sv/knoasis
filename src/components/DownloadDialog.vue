<script setup>
// 学科包下载（弹窗）
// - 左侧窄列：学科列表（点击切换 + 选中高亮）
// - 右侧：当前学科的版本包列表（版本号 / 条目数 / 包大小 / 更新日期 + [下载]）
// - 打开时优先从多个镜像源读取学科包目录（raw → jsDelivr → ghproxy → gitmirror，依次回退）；
//   拉取成功则刷新本地缓存（4 小时内有效）；全部镜像失败则回退到本地缓存。
// - 点击下载时从 pkg.path 经同样的多镜像源拉取 .kpkg，落盘临时文件后调用 knowledge.importSet 导入并 reload。
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useUi } from "../stores/ui.js";
import { isTauri, knowledge } from "../lib/ipc.js";
import { useKnowledgeSets } from "../stores/knowledgeSets.js";
import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
import { writeFile, readTextFile, writeTextFile } from "@tauri-apps/plugin-fs";
import { tempDir, join, appDataDir } from "@tauri-apps/api/path";

const ui = useUi();
const knowledgeSets = useKnowledgeSets();

// 学科包目录与包文件：经多个镜像源依次尝试拉取。
// 国内 raw.githubusercontent.com 常被墙，故同时提供 jsDelivr / ghproxy / gitmirror 回退。
// catalog.json 与 .kpkg 均位于仓库根目录，用仓库相对路径拼接各镜像基址。
const REPO = "william-sv/knoasis";
const BRANCH = "main";
const MIRROR_TEMPLATES = [
  (p) => `https://cdn.jsdelivr.net/gh/${REPO}@${BRANCH}/${p}`,
  (p) => `https://ghproxy.com/https://raw.githubusercontent.com/${REPO}/${BRANCH}/${p}`,
  (p) => `https://raw.gitmirror.com/${REPO}/${BRANCH}/${p}`,
  (p) => `https://raw.githubusercontent.com/${REPO}/${BRANCH}/${p}`,
];
function candidatesFor(relPath) {
  return MIRROR_TEMPLATES.map((t) => t(relPath));
}
const CATALOG_CANDIDATES = candidatesFor("disciplines/catalog.json");

// 单请求超时封装：被墙的域名不会等到 TCP 超时（几十秒~分钟）才抛错，
// 而是到时立即放弃，让上层切换下一个镜像源或回退缓存，避免界面长时间卡在 loading。
function withTimeout(promise, ms) {
  return new Promise((resolve, reject) => {
    const t = setTimeout(() => reject(new Error(`请求超时（${ms}ms）`)), ms);
    promise.then(
      (v) => {
        clearTimeout(t);
        resolve(v);
      },
      (e) => {
        clearTimeout(t);
        reject(e);
      },
    );
  });
}

// 依次尝试候选地址，返回首个成功的 JSON；全部失败则抛出最后一个错误。
async function fetchFirstJson(candidates) {
  let lastErr;
  for (const url of candidates) {
    try {
      const resp = await withTimeout(tauriFetch(url), 8000);
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
      return await resp.json();
    } catch (e) {
      lastErr = e;
    }
  }
  throw lastErr || new Error("所有镜像源均不可用");
}

// 本地缓存：目录拉取失败时回退使用，下载后记录时间戳，4 小时内有效。
const CACHE_FILE = "catalog.cache.json";
const CACHE_TTL_MS = 4 * 60 * 60 * 1000; // 4 小时

const subjects = ref([]);
const loadingCatalog = ref(false);
const catalogError = ref("");
const usingCache = ref(false);

const activeId = ref(null);
const activeSubject = computed(
  () => subjects.value.find((s) => s.id === activeId.value) || subjects.value[0] || null,
);

/** 正在下载的包 key（`${subjectId}:${version}`），同时只允许一个进行中 */
const downloading = ref("");

function isDownloading(subject, pkg) {
  return downloading.value === `${subject.id}:${pkg.version}`;
}

async function cachePath() {
  const dir = await appDataDir();
  return await join(dir, CACHE_FILE);
}

async function saveCatalogCache(data) {
  const p = await cachePath();
  await writeTextFile(p, JSON.stringify({ ...data, _cachedAt: Date.now() }));
}

async function readCatalogCache() {
  try {
    const p = await cachePath();
    const text = await readTextFile(p);
    const data = JSON.parse(text);
    if (data && typeof data._cachedAt === "number") return data;
    return null; // 结构异常
  } catch {
    return null; // 不存在 / 解析失败
  }
}

async function loadCatalog() {
  if (!isTauri()) {
    catalogError.value = "下载模块仅在桌面应用中可用";
    return;
  }
  loadingCatalog.value = true;
  catalogError.value = "";
  usingCache.value = false;

  // 1) 先看本地缓存：4 小时内新鲜则直接使用，不向云端发请求
  const cached = await readCatalogCache();
  const fresh = cached && Date.now() - cached._cachedAt <= CACHE_TTL_MS;
  if (cached && fresh) {
    subjects.value = cached.subjects || [];
    activeId.value = subjects.value[0]?.id ?? null;
    usingCache.value = true;
    loadingCatalog.value = false;
    return;
  }

  // 2) 缓存缺失或已过期：向云端（多镜像）拉取最新目录并刷新时间戳
  try {
    const data = await fetchFirstJson(CATALOG_CANDIDATES);
    subjects.value = data.subjects || [];
    activeId.value = subjects.value[0]?.id ?? null;
    try {
      await saveCatalogCache(data);
    } catch {
      /* 缓存写入失败不阻塞在线读取 */
    }
  } catch (e) {
    // 云端也失败：若有过期缓存则降级使用，否则报错
    if (cached) {
      subjects.value = cached.subjects || [];
      activeId.value = subjects.value[0]?.id ?? null;
      usingCache.value = true;
    } else {
      subjects.value = [];
      catalogError.value = "学科目录加载失败（已尝试多个镜像源），请检查网络后重试";
    }
  } finally {
    loadingCatalog.value = false;
  }
}

async function doDownload(subject, pkg) {
  if (downloading.value) return;
  downloading.value = `${subject.id}:${pkg.version}`;
  try {
    const candidates = candidatesFor(pkg.path);
    let buf = null;
    let lastErr;
    for (const url of candidates) {
      try {
        const resp = await withTimeout(tauriFetch(url), 20000);
        if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
        buf = await resp.arrayBuffer();
        break;
      } catch (e) {
        lastErr = e;
      }
    }
    if (!buf) throw lastErr || new Error("所有镜像源均不可用");
    const dir = await tempDir();
    const path = await join(dir, `${pkg.id}.kpkg`);
    await writeFile(path, new Uint8Array(buf));
    await knowledge.importSet({ src_dir: path });
    await knowledgeSets.reload();
    ui.showToast(`已下载并导入 ${subject.name} ${pkg.version}`);
  } catch (e) {
    const msg = e && e.message ? e.message : String(e);
    ui.showToast(`下载失败：${msg}`);
  } finally {
    downloading.value = "";
  }
}

function onKeydown(e) {
  if (e.key === "Escape") ui.closeDownload();
}

onMounted(() => {
  window.addEventListener("keydown", onKeydown);
  loadCatalog();
});
onBeforeUnmount(() => window.removeEventListener("keydown", onKeydown));
</script>

<template>
  <div class="scrim" @click="ui.closeDownload()">
    <div class="modal" @click.stop>
      <header class="modal-head">
        <div class="modal-head-main">
          <div class="modal-title">学科包下载</div>
          <div class="modal-sub">
            浏览可用的学科知识包<span v-if="usingCache"> · 离线缓存</span>
          </div>
        </div>
        <!-- 关闭按钮固定右上角 -->
        <button class="icon-btn modal-close" title="关闭" @click="ui.closeDownload()">✕</button>
      </header>

      <div v-if="loadingCatalog" class="modal-body modal-body--empty">
        正在加载学科目录…
      </div>
      <div v-else-if="catalogError" class="modal-body modal-body--empty">
        {{ catalogError }}
      </div>
      <div v-else-if="activeSubject" class="modal-body">
        <!-- 左侧学科列表 -->
        <aside class="subjects">
          <button
            v-for="s in subjects"
            :key="s.id"
            class="subject-item"
            :class="{ 'is-active': s.id === activeId }"
            @click="activeId = s.id"
          >
            <span class="dot" :style="{ background: s.color }"></span>
            <span class="subject-name">{{ s.name }}</span>
          </button>
        </aside>

        <!-- 右侧版本包列表 -->
        <section class="packages">
          <div v-for="pkg in activeSubject.packages" :key="pkg.version" class="pkg-row">
            <div class="pkg-main">
              <div class="pkg-title">
                <span class="pkg-version">{{ pkg.version }}</span>
              </div>
              <div class="pkg-meta">
                {{ pkg.entries }} 条 · {{ pkg.size }} · 更新于 {{ pkg.updatedAt }}
              </div>
            </div>
            <button
              class="btn btn--dl"
              :disabled="Boolean(downloading)"
              @click="doDownload(activeSubject, pkg)"
            >
              {{ isDownloading(activeSubject, pkg) ? "下载中…" : "下载" }}
            </button>
          </div>

          <div v-if="!activeSubject.packages.length" class="empty">
            该学科暂无可下载的版本包。
          </div>
        </section>
      </div>

      <div v-else class="modal-body modal-body--empty">
        暂无可用学科包。
      </div>
    </div>
  </div>
</template>

<style scoped>
.scrim {
  position: absolute;
  inset: 0;
  z-index: 150;
  background: var(--scrim);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
}

.modal {
  position: relative;
  width: min(700px, 100%);
  height: min(540px, 100%);
  display: flex;
  flex-direction: column;
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  box-shadow: var(--shadow-2);
  overflow: hidden;
}

/* 头部 */
.modal-head {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  padding: 14px 46px 12px 16px;
  border-bottom: 1px solid var(--border);
  flex: none;
}
.modal-head-main {
  min-width: 0;
}
.modal-title {
  font-size: 14.5px;
  font-weight: 650;
  color: var(--text);
}
.modal-sub {
  margin-top: 4px;
  font-size: 11.5px;
  color: var(--text-faint);
}
.modal-close {
  position: absolute;
  top: 10px;
  right: 10px;
}

/* 主体：左列 + 右列 */
.modal-body {
  flex: 1;
  min-height: 0;
  display: flex;
}

.subjects {
  flex: none;
  width: 180px;
  min-width: 0;
  overflow: auto;
  border-right: 1px solid var(--border);
  background: var(--panel-inset);
  padding: 8px;
}
.subject-item {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 8px 10px;
  border-radius: var(--radius-sm);
  font-size: 12.5px;
  color: var(--text);
  text-align: left;
}
.subject-item + .subject-item {
  margin-top: 2px;
}
.subject-item:hover {
  background: var(--panel-hover);
}
.subject-item.is-active {
  background: rgba(var(--accent-rgb), 0.12);
  color: var(--accent);
  font-weight: 600;
}
.subject-item .dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex: none;
}
.subject-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.packages {
  flex: 1;
  min-width: 0;
  overflow: auto;
  padding: 10px 14px 14px;
}
.pkg-row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 12px;
  border-radius: var(--radius-sm);
}
.pkg-row + .pkg-row {
  border-top: 1px solid var(--border-soft);
}
.pkg-row:hover {
  background: var(--panel-hover);
}
.pkg-main {
  flex: 1;
  min-width: 0;
}
.pkg-version {
  font-size: 13px;
  font-weight: 600;
  color: var(--text);
}
.pkg-meta {
  margin-top: 4px;
  font-size: 11.5px;
  color: var(--text-faint);
}

/* 按钮 */
.btn {
  height: 28px;
  padding: 0 14px;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border);
  background: var(--panel);
  color: var(--text);
  font-size: 12px;
  white-space: nowrap;
  transition: background 0.12s ease, border-color 0.12s ease, color 0.12s ease;
}
.btn:hover:not(:disabled) {
  background: var(--panel-hover);
}
.btn:disabled {
  opacity: 0.55;
  cursor: default;
}
.btn--dl {
  background: rgba(var(--accent-rgb), 0.12);
  border-color: rgba(var(--accent-rgb), 0.34);
  color: var(--accent);
}
.btn--dl:hover:not(:disabled) {
  background: rgba(var(--accent-rgb), 0.2);
}

/* 图标按钮（关闭） */
.icon-btn {
  width: 26px;
  height: 26px;
  border-radius: var(--radius-sm);
  color: var(--text-muted);
  font-size: 12px;
  flex: none;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}
.icon-btn:hover {
  background: var(--panel-hover);
  color: var(--text);
}

.empty {
  padding: 40px 16px;
  text-align: center;
  font-size: 12px;
  color: var(--text-faint);
}

/* 无可用目录 / 加载中 / 错误 空态 */
.modal-body--empty {
  align-items: center;
  justify-content: center;
  font-size: 13px;
  color: var(--text-faint);
  text-align: center;
}
</style>
