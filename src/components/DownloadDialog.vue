<script setup>
// 学科包下载（弹窗）
// - 左侧窄列：学科列表（点击切换 + 选中高亮）
// - 右侧：当前学科的版本包列表（版本号 / 条目数 / 包大小 / 更新日期 + [下载]）
// - 真实下载源接入前：SUBJECTS 为空，弹窗展示「下载源尚未接入」空态，不展示任何假数据。
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useUi } from "../stores/ui.js";

const ui = useUi();

// 学科包目录：真实数据由后端/下载接口填充；当前无可用目录（不内置任何示例/假数据）。
const SUBJECTS = [];

const activeId = ref(SUBJECTS[0]?.id ?? null);
const activeSubject = computed(
  () => SUBJECTS.find((s) => s.id === activeId.value) || SUBJECTS[0] || null,
);

/** 正在下载的包 key（`${subjectId}:${version}`），同时只允许一个进行中 */
const downloading = ref("");

function isDownloading(subject, pkg) {
  return downloading.value === `${subject.id}:${pkg.version}`;
}

function doDownload(subject, pkg) {
  if (downloading.value) return;
  downloading.value = `${subject.id}:${pkg.version}`;
  // 接入真实下载源前的占位交互：0.8s 后提示尚未接入
  setTimeout(() => {
    downloading.value = "";
    ui.showToast("下载源尚未接入，敬请期待");
  }, 800);
}

function onKeydown(e) {
  if (e.key === "Escape") ui.closeDownload();
}

// 挂载/卸载时绑定 Esc 关闭
onMounted(() => window.addEventListener("keydown", onKeydown));
onBeforeUnmount(() => window.removeEventListener("keydown", onKeydown));
</script>

<template>
  <div class="scrim" @click="ui.closeDownload()">
    <div class="modal" @click.stop>
      <header class="modal-head">
        <div class="modal-head-main">
          <div class="modal-title">学科包下载</div>
          <div class="modal-sub">浏览可用的学科知识包</div>
        </div>
        <!-- 关闭按钮固定右上角 -->
        <button class="icon-btn modal-close" title="关闭" @click="ui.closeDownload()">✕</button>
      </header>

      <div v-if="activeSubject" class="modal-body">
        <!-- 左侧学科列表 -->
        <aside class="subjects">
          <button
            v-for="s in SUBJECTS"
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
        下载源尚未接入，学科包目录即将上线。
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
  background: color-mix(in srgb, var(--accent) 12%, var(--panel));
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
  background: color-mix(in srgb, var(--accent) 12%, var(--panel));
  border-color: color-mix(in srgb, var(--accent) 34%, var(--border));
  color: var(--accent);
}
.btn--dl:hover:not(:disabled) {
  background: color-mix(in srgb, var(--accent) 20%, var(--panel));
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

/* 无可用目录（下载源尚未接入）空态 */
.modal-body--empty {
  align-items: center;
  justify-content: center;
  font-size: 13px;
  color: var(--text-faint);
  text-align: center;
}
</style>
