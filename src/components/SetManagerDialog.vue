<script setup>
// 学科包管理（Dash 式 Docsets 管理）
// - 列出全部已发现学科包（含已停用的），展示来源（内置 / 用户）、版本、条目数、安装路径
// - 导入：选择一个 *.kpkg 文件（Studio 导出的单文件包）→ 校验 + 复制到用户知识根（也兼容手输 .knowledgeset 目录路径）
// - 删除：用户包真删（内置同名包则回落）；内置包不可删，退化为「停用」
// - 启用/停用：停用后不出现在学科列表与搜索中，但保留在管理面板可随时恢复
// 任何变更后自动重载学科数据（knowledge_reload + 重新拉取），无需重启应用。
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useUi } from "../stores/ui.js";
import { useKnowledgeSets } from "../stores/knowledgeSets.js";
import { isTauri, knowledge } from "../lib/ipc.js";

const ui = useUi();
const ks = useKnowledgeSets();

const loading = ref(false);
const errorText = ref("");
const userRoot = ref("");
const sets = ref([]);
const busyId = ref(""); // 正在执行启用/删除操作的包 id
const confirmId = ref(""); // 待二次确认删除的包 id
const showManualImport = ref(false);
const manualPath = ref("");

const enabledCount = computed(() => sets.value.filter((s) => s.enabled).length);
const disabledCount = computed(() => sets.value.length - enabledCount.value);

async function load() {
  if (!isTauri()) {
    errorText.value = "浏览器预览不支持管理学科包，请在桌面应用中使用。";
    return;
  }
  loading.value = true;
  errorText.value = "";
  try {
    const res = await knowledge.manageList();
    userRoot.value = res.user_root || "";
    sets.value = Array.isArray(res.sets) ? res.sets : [];
  } catch (e) {
    errorText.value = e?.message || String(e);
  } finally {
    loading.value = false;
  }
}

/** 变更生效：重载学科数据；若当前选中学科已被删除/停用则回到「全部学科」 */
async function applyChanges() {
  const res = await ks.reload();
  if (!res.ok) {
    ui.showToast(`学科包已变更，但重新加载失败：${res.error || "未知错误"}`, 3200);
    return;
  }
  if (ui.activeSetId !== "all" && !ks.sets.some((s) => s.id === ui.activeSetId)) {
    ui.switchSet("all");
  }
}

async function afterChange(message) {
  await load();
  await applyChanges();
  if (message) ui.showToast(message);
}

async function toggleEnabled(set) {
  if (!set.id || busyId.value) return;
  busyId.value = set.id;
  try {
    const res = await knowledge.setEnabled({ set_id: set.id, enabled: !set.enabled });
    sets.value = Array.isArray(res.sets) ? res.sets : [];
    await applyChanges();
    ui.showToast(set.enabled ? `已停用「${set.name}」` : `已启用「${set.name}」`);
  } catch (e) {
    ui.showToast(`操作失败：${e?.message || e}`, 3000);
  } finally {
    busyId.value = "";
  }
}

/**
 * 设为「当前使用的学科包」（与启用开关相互独立）：
 * 点击后把 ui.activeSetId 切到该包（持久化 + 切到浏览态 + 自动选中首条），并提示。
 * 停用的包不参与浏览，故若目标包处于停用态，先启用再切换。
 */
async function activate(set) {
  if (!set.id || busyId.value) return;
  if (set.id === ui.activeSetId) {
    ui.showToast(`「${set.name}」就是当前使用的学科包`);
    return;
  }
  busyId.value = set.id;
  try {
    if (!set.enabled) {
      const res = await knowledge.setEnabled({ set_id: set.id, enabled: true });
      sets.value = Array.isArray(res.sets) ? res.sets : sets.value;
    }
    ui.switchSet(set.id);
    ui.showToast(`当前使用的学科包已更换为「${set.name}」`);
  } catch (e) {
    ui.showToast(`操作失败：${e?.message || e}`, 3000);
  } finally {
    busyId.value = "";
  }
}

async function doRemove(set) {
  if (!set.id || busyId.value) return;
  busyId.value = set.id;
  confirmId.value = "";
  try {
    const res = await knowledge.removeSet({ set_id: set.id });
    let msg;
    if (res.removed) {
      msg = res.fell_back_to_builtin
        ? `已删除「${set.name}」，回落到内置版本`
        : `已删除「${set.name}」`;
    } else {
      msg = `「${set.name}」为内置学科包，已改为停用（收藏与笔记保留）`;
    }
    await afterChange(msg);
  } catch (e) {
    ui.showToast(`删除失败：${e?.message || e}`, 3000);
  } finally {
    busyId.value = "";
  }
}

async function pickAndImport() {
  if (showManualImport.value && !manualPath.value.trim()) return;
  let src = manualPath.value.trim();
  if (!src) {
    try {
      // 动态引入：未安装 dialog 插件时不阻塞页面加载
      const { open } = await import("@tauri-apps/plugin-dialog");
      const picked = await open({
        multiple: false,
        filters: [{ name: "Knoasis 学科包", extensions: ["kpkg"] }],
        title: "选择学科包文件（*.kpkg）",
      });
      if (!picked) return;
      src = Array.isArray(picked) ? String(picked[0]) : String(picked);
    } catch (e) {
      // 目录选择器不可用 → 退回手输路径
      showManualImport.value = true;
      ui.showToast("无法打开目录选择器，请手动输入学科包路径", 2600);
      return;
    }
  }
  loading.value = true;
  try {
    const res = await knowledge.importSet({ src });
    manualPath.value = "";
    showManualImport.value = false;
    await afterChange(
      `已导入「${res.name}」${res.entry_count} 条${res.replaced ? "（覆盖旧版本）" : ""}`,
    );
  } catch (e) {
    errorText.value = `导入失败：${e?.message || e}`;
  } finally {
    loading.value = false;
  }
}

async function openUserRoot() {
  if (!userRoot.value) return;
  try {
    // 走后端命令在系统文件管理器中打开（越界路径由 Rust 侧拒绝）
    await knowledge.openDir(userRoot.value);
  } catch (e) {
    ui.showToast(`无法打开目录：${e?.message || e}`, 2600);
  }
}

function onKeydown(e) {
  if (e.key === "Escape" && !confirmId.value) ui.closeSetsManager();
}

onMounted(() => {
  load();
  window.addEventListener("keydown", onKeydown);
});
onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKeydown);
});
</script>

<template>
  <div class="scrim" @click="ui.closeSetsManager()">
    <div class="modal" @click.stop>
      <header class="modal-head">
        <div class="modal-head-main">
          <div class="modal-title">学科包管理</div>
          <div class="modal-sub">
            共 {{ sets.length }} 个学科包 · 已启用 {{ enabledCount }}
            <template v-if="disabledCount"> · 已停用 {{ disabledCount }}</template>
          </div>
        </div>
        <!-- 关闭按钮固定在弹窗右上角 -->
        <button class="icon-btn modal-close" title="关闭" @click="ui.closeSetsManager()">✕</button>
      </header>

      <div class="modal-actions">
        <button class="btn btn--primary" :disabled="loading" @click="pickAndImport">
          导入学科包…
        </button>
        <button class="btn" :disabled="loading" @click="load">刷新</button>
        <span v-if="loading" class="hint">处理中…</span>
        <span class="spacer"></span>
        <span class="hint">停用的学科包不参与浏览与搜索，可随时重新启用</span>
      </div>

      <div v-if="showManualImport" class="manual">
        <input
          v-model="manualPath"
          class="manual-input"
          type="text"
          spellcheck="false"
          placeholder="输入 .kpkg 文件或 .knowledgeset 目录绝对路径，例如 /Users/you/Downloads/example.kpkg"
          @keydown.enter="pickAndImport"
        />
        <button class="btn" :disabled="!manualPath.trim() || loading" @click="pickAndImport">
          导入
        </button>
      </div>

      <div v-if="errorText" class="alert">{{ errorText }}</div>

      <div class="list">
        <div v-if="!sets.length && !loading" class="empty">
          暂无学科包。可导入 *.kpkg 文件（Studio 导出的单文件包），或把包放入下方学科包目录后刷新。
        </div>

        <div
          v-for="s in sets"
          :key="s.id"
          class="row"
          :class="{ 'is-off': !s.enabled, 'is-busy': busyId === s.id, 'is-active': s.id === ui.activeSetId }"
        >
          <button
            class="switch"
            :class="{ 'is-on': s.enabled }"
            :title="s.enabled ? '点击停用' : '点击启用'"
            :disabled="Boolean(busyId)"
            @click="toggleEnabled(s)"
          >
            <span class="switch-knob"></span>
          </button>

          <div class="row-main">
            <div class="row-title">
              <span class="dot" :style="{ background: s.color || 'var(--accent)' }"></span>
              <span class="name">{{ s.name }}</span>
              <span class="badge">v{{ s.version }}</span>
              <span class="badge badge--ghost">{{ s.origin === "builtin" ? "内置" : "用户" }}</span>
              <span v-if="!s.enabled" class="badge badge--off">已停用</span>
              <span v-if="s.id === ui.activeSetId" class="badge badge--active">当前</span>
            </div>
            <div class="row-meta">
              {{ s.entry_count }} 条 · {{ s.id }} ·
              <span class="path" :title="s.dir">{{ s.dir }}</span>
            </div>
            <div v-if="s.description" class="row-desc">{{ s.description }}</div>
          </div>

          <div class="row-side">
            <button
              class="btn btn--ghost"
              :disabled="Boolean(busyId) || s.id === ui.activeSetId"
              :title="s.id === ui.activeSetId ? '当前正在使用的学科包' : '设为当前使用的学科包'"
              @click="activate(s)"
            >{{ s.id === ui.activeSetId ? "当前使用中" : "设为当前" }}</button>
            <template v-if="confirmId === s.id">
              <button class="btn btn--danger" @click="doRemove(s)">确认删除</button>
              <button class="btn" @click="confirmId = ''">取消</button>
            </template>
            <button
              v-else
              class="btn btn--ghost"
              :disabled="Boolean(busyId)"
              :title="s.removable ? '删除该学科包' : s.enabled ? '停用该学科包' : '启用该学科包'"
              @click="s.removable ? (confirmId = s.id) : toggleEnabled(s)"
            >
              {{ s.removable ? "删除" : s.enabled ? "停用" : "启用" }}
            </button>
          </div>
        </div>
      </div>

      <footer class="modal-foot">
        <span class="foot-label">学科包目录</span>
        <span class="foot-path" :title="userRoot">{{ userRoot || "（未加载）" }}</span>
        <button class="btn btn--ghost" :disabled="!userRoot" @click="openUserRoot">
          打开目录
        </button>
      </footer>
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
  width: min(760px, 100%);
  max-height: 100%;
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
  /* 右侧留白给绝对定位的关闭按钮 */
  padding: 14px 46px 12px 16px;
  border-bottom: 1px solid var(--border);
  flex: none;
}
.modal-head-main {
  min-width: 0;
}
/* 关闭按钮固定右上角 */
.modal-close {
  position: absolute;
  top: 10px;
  right: 10px;
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

/* 操作条 */
.modal-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 16px;
  border-bottom: 1px solid var(--border);
  flex: none;
}
.spacer {
  flex: 1;
}
.hint {
  font-size: 11.5px;
  color: var(--text-faint);
}

.manual {
  display: flex;
  gap: 8px;
  padding: 10px 16px;
  border-bottom: 1px solid var(--border);
  flex: none;
}
.manual-input {
  flex: 1;
  min-width: 0;
  height: 30px;
  padding: 0 10px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--field-bg);
  color: var(--text);
  font-family: var(--font-mono);
  font-size: 11.5px;
}
.manual-input:focus {
  outline: none;
  border-color: color-mix(in srgb, var(--accent) 50%, var(--border));
  background: var(--panel);
}

.alert {
  margin: 10px 16px 0;
  padding: 8px 10px;
  border-radius: var(--radius-sm);
  background: color-mix(in srgb, #e5484d 12%, var(--panel));
  color: var(--text);
  font-size: 11.5px;
  line-height: 1.6;
  word-break: break-word;
}

/* 列表 */
.list {
  flex: 1;
  min-height: 120px;
  overflow: auto;
  padding: 8px 12px 12px;
}
.empty {
  padding: 28px 16px;
  text-align: center;
  font-size: 12px;
  color: var(--text-faint);
  line-height: 1.7;
}

.row {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  padding: 10px 10px;
  border-radius: var(--radius-sm);
}
.row + .row {
  border-top: 1px solid var(--border-soft);
}
.row:hover {
  background: var(--panel-hover);
}
.row.is-off .row-main {
  opacity: 0.55;
}
.row.is-busy {
  opacity: 0.6;
}

/* 当前激活的学科包：高亮底色 + 左侧强调条 */
.row.is-active {
  background: color-mix(in srgb, var(--accent) 10%, var(--panel));
  box-shadow: inset 3px 0 0 var(--accent);
}
.row.is-active:hover {
  background: color-mix(in srgb, var(--accent) 14%, var(--panel));
}
.row.is-active.is-off {
  /* 理论上激活的包必为启用态（activate 会先启用），这里仅兜底避免样式冲突 */
  opacity: 1;
}

/* 开关 */
.switch {
  flex: none;
  width: 34px;
  height: 19px;
  margin-top: 2px;
  padding: 2px;
  border-radius: 999px;
  background: var(--field-bg-hover);
  border: 1px solid var(--border);
  display: flex;
  transition: background 0.15s ease, border-color 0.15s ease;
}
.switch.is-on {
  background: var(--accent);
  border-color: var(--accent);
}
.switch-knob {
  width: 13px;
  height: 13px;
  border-radius: 50%;
  background: #fff;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.2);
  transition: transform 0.15s ease;
}
.switch.is-on .switch-knob {
  transform: translateX(15px);
}

.row-main {
  flex: 1;
  min-width: 0;
}
.row-title {
  display: flex;
  align-items: center;
  gap: 7px;
  flex-wrap: wrap;
}
.dot {
  width: 9px;
  height: 9px;
  border-radius: 50%;
  flex: none;
}
.name {
  font-size: 13px;
  font-weight: 600;
  color: var(--text);
}
.badge {
  padding: 1px 6px;
  border-radius: 999px;
  font-size: 10.5px;
  color: var(--chip-text);
  background: var(--chip-bg);
}
.badge--ghost {
  background: transparent;
  border: 1px solid var(--border);
  color: var(--text-muted);
}
.badge--off {
  background: color-mix(in srgb, var(--text-muted) 18%, transparent);
  color: var(--text-muted);
}
.badge--active {
  background: color-mix(in srgb, var(--accent) 16%, transparent);
  color: var(--accent);
  border: 1px solid color-mix(in srgb, var(--accent) 45%, transparent);
}
.row-meta {
  margin-top: 4px;
  font-size: 11px;
  color: var(--text-faint);
}
.path {
  font-family: var(--font-mono);
  max-width: 340px;
  display: inline-block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  vertical-align: bottom;
}
.row-desc {
  margin-top: 3px;
  font-size: 11.5px;
  color: var(--text-muted);
  line-height: 1.5;
}

.row-side {
  flex: none;
  display: flex;
  align-items: center;
  gap: 6px;
}

/* 按钮 */
.btn {
  height: 28px;
  padding: 0 11px;
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
  opacity: 0.5;
  cursor: default;
}
.btn--primary {
  background: var(--accent);
  border-color: var(--accent);
  color: #fff;
}
.btn--primary:hover:not(:disabled) {
  filter: brightness(1.06);
  background: var(--accent);
}
.btn--ghost {
  background: transparent;
  color: var(--text-muted);
}
.btn--danger {
  background: #e5484d;
  border-color: #e5484d;
  color: #fff;
}
.icon-btn {
  width: 26px;
  height: 26px;
  border-radius: var(--radius-sm);
  color: var(--text-muted);
  font-size: 12px;
  flex: none;
}
.icon-btn:hover {
  background: var(--panel-hover);
  color: var(--text);
}

/* 底部 */
.modal-foot {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 9px 16px;
  border-top: 1px solid var(--border);
  background: var(--panel-inset);
  flex: none;
}
.foot-label {
  font-size: 11px;
  color: var(--text-faint);
  flex: none;
}
.foot-path {
  flex: 1;
  min-width: 0;
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--text-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
