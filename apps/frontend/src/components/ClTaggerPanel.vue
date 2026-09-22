<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, ref } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  clTaggerDownload,
  clTaggerOpenDir,
  clTaggerStatus,
  gpuDetect,
  type ClTaggerStatus,
  type GpuDetectResult,
} from "@/api/tauri";
import { useAppStore } from "@/stores/app";
import agreeShot from "@/assets/help/hf-agree.jpg";
import tokenShot from "@/assets/help/hf-token-create.jpg";

const store = useAppStore();
const gpu = ref<GpuDetectResult | null>(null);
const status = ref<ClTaggerStatus | null>(null);
const hfDraft = ref("");
const message = ref("");
const busy = ref(false);
const downloadPct = ref(0);
const downloadMsg = ref("");
const helpOpen = ref(false);
const helpBtn = ref<HTMLButtonElement | null>(null);
const helpStyle = ref<Record<string, string>>({});
let unlisten: UnlistenFn | undefined;
let helpTimer = 0;

function placeHelp() {
  const el = helpBtn.value;
  if (!el) return;
  const rect = el.getBoundingClientRect();
  const width = 380;
  let left = rect.right + 12;
  if (left + width > window.innerWidth - 12) left = Math.max(12, rect.left - width - 12);
  const maxHeight = Math.min(560, window.innerHeight - 24);
  let top = rect.top - 4;
  if (top + maxHeight > window.innerHeight - 12) top = Math.max(12, window.innerHeight - 12 - maxHeight);
  helpStyle.value = { top: `${top}px`, left: `${left}px`, width: `${width}px`, maxHeight: `${maxHeight}px` };
}

function showHelp() {
  window.clearTimeout(helpTimer);
  helpOpen.value = true;
  void nextTick(placeHelp);
}

function hideHelp() {
  window.clearTimeout(helpTimer);
  helpTimer = window.setTimeout(() => {
    helpOpen.value = false;
  }, 180);
}

function formatErr(e: unknown) {
  if (typeof e === "string") return e;
  if (e && typeof e === "object" && "message" in e) return String((e as { message: unknown }).message);
  return String(e);
}

async function refresh() {
  try {
    gpu.value = await gpuDetect();
    status.value = await clTaggerStatus();
  } catch (e) {
    message.value = formatErr(e);
  }
}

async function persistHf() {
  const next = hfDraft.value.trim();
  if (next) store.settings.huggingfaceToken = next;
  await store.saveSettings();
  hfDraft.value = "";
}

async function toggleLocal(enabled: boolean) {
  if (enabled && gpu.value && !gpu.value.usable) {
    store.settings.localClTaggerEnabled = false;
    message.value = gpu.value.reason;
    return;
  }
  store.settings.localClTaggerEnabled = enabled;
  try {
    await persistHf();
    message.value = enabled ? "已启用本地 CL Tagger v2。" : "已关闭本地打标，反推仍可走 Hugging Face 空间。";
    await refresh();
  } catch (e) {
    store.settings.localClTaggerEnabled = false;
    message.value = formatErr(e);
  }
}

async function saveThreshold() {
  try {
    await store.saveSettings();
  } catch (e) {
    message.value = formatErr(e);
  }
}

async function download() {
  if (busy.value) return;
  const typed = hfDraft.value.trim().replace(/^Bearer\s+/i, "");
  if (!typed && store.settings.huggingfaceToken !== "configured") {
    message.value = "先把 Hugging Face token 填在上面。打开 huggingface.co/cella110n/cl_tagger_v2，登录后同意许可，再粘贴 hf_ 开头的 token。";
    return;
  }
  busy.value = true;
  downloadPct.value = 1;
  downloadMsg.value = "开始下载模型（约 1GB+，只保存在本机）…";
  message.value = downloadMsg.value;
  try {
    if (typed) store.settings.huggingfaceToken = typed;
    await store.saveSettings();
    status.value = await clTaggerDownload(typed || undefined);
    hfDraft.value = "";
    downloadPct.value = 100;
    message.value = `已下载 CL Tagger ${status.value.version} 到本机。`;
  } catch (e) {
    message.value = formatErr(e);
  } finally {
    busy.value = false;
    downloadMsg.value = "";
  }
}

onMounted(async () => {
  unlisten = await listen<{ file: string; percent: number; message: string }>("cl-tagger-download", (event) => {
    downloadPct.value = Math.round(event.payload.percent);
    downloadMsg.value = event.payload.message;
    message.value = event.payload.message;
  });
  await refresh();
});

onUnmounted(() => {
  window.clearTimeout(helpTimer);
  void unlisten?.();
});
</script>

<template>
  <div class="panel">
    <p class="hint">
      第一次安装会检测 NVIDIA 显卡。显存足够才允许启用本地
      <code>cl_tagger_v2</code>（v2_00 ONNX）。权重有许可限制，程序不会内置；开发时下到项目 <code>models/</code>，安装后下到程序安装目录的 <code>models/</code>。
    </p>
    <div class="card-lite">
      <strong>显卡</strong>
      <p class="hint">{{ gpu?.reason || "正在检测…" }}</p>
      <ul v-if="gpu?.gpus.length" class="gpus">
        <li v-for="item in gpu.gpus" :key="item.name">
          {{ item.name }} · {{ item.vramMb }} MB · 驱动 {{ item.driver || "—" }}
        </li>
      </ul>
      <button class="btn" type="button" @click="refresh">重新检测</button>
    </div>
    <div class="field">
      <div class="token-label">
        <span>Hugging Face token</span>
        <button
          ref="helpBtn"
          class="help"
          type="button"
          aria-label="Hugging Face token 教程"
          @mouseenter="showHelp"
          @mouseleave="hideHelp"
          @focus="showHelp"
          @blur="hideHelp"
        >
          ?
        </button>
      </div>
      <input
        v-model="hfDraft"
        type="password"
        autocomplete="off"
        :placeholder="store.settings.huggingfaceToken === 'configured' ? '已配置，输入新 token 可覆盖' : 'hf_... 只保存在本机'"
      />
    </div>
    <label class="check" :class="{ off: gpu && !gpu.usable }">
      <input
        :checked="store.settings.localClTaggerEnabled"
        :disabled="!gpu?.usable"
        type="checkbox"
        @change="toggleLocal(($event.target as HTMLInputElement).checked)"
      />
      启用本地 CL Tagger v2
    </label>
    <p v-if="gpu && !gpu.usable" class="hint">没有可用 NVIDIA 显卡时，请继续用反推页的官方空间。</p>
    <label class="slide">
      <span>默认阈值 <b>{{ store.settings.localClTaggerThreshold.toFixed(2) }}</b></span>
      <input
        v-model.number="store.settings.localClTaggerThreshold"
        type="range"
        min="0.05"
        max="0.95"
        step="0.05"
        @change="saveThreshold"
      />
    </label>
    <p class="hint">
      模型状态：{{ status?.downloaded ? `已下载 ${status.version}` : status ? `未下载（缺 ${status.missing.join("、") || "文件"}）` : "—" }}
    </p>
    <p v-if="status?.dir" class="hint path">{{ status.dir }}</p>
    <div v-if="busy || downloadMsg" class="progress">
      <span>{{ downloadMsg || "下载中…" }}</span>
      <i :style="{ width: `${Math.max(4, downloadPct)}%` }" />
    </div>
    <div class="row">
      <button class="btn primary" type="button" :disabled="busy" @click="download">
        {{ busy ? "下载中…" : status?.downloaded ? "重新下载模型" : "下载模型到本机" }}
      </button>
      <button class="btn" type="button" @click="clTaggerOpenDir()">打开模型目录</button>
    </div>
    <p v-if="message" class="note">{{ message }}</p>
    <Teleport to="body">
      <div
        v-if="helpOpen"
        class="cl-token-help"
        :style="helpStyle"
        @mouseenter="showHelp"
        @mouseleave="hideHelp"
      >
        <strong>怎么拿到 Hugging Face token</strong>
        <ol>
          <li>打开模型页，用准备创建 token 的账号登录，点 Agree 同意许可。没同意过，下载会返回 403。</li>
          <li>打开 Access Tokens，点 New token。名字随意，Role 选 read，再点 Generate a token。</li>
          <li>如果只有 Fine-grained：勾选读取已同意的公开 gated 仓库，或只给这个模型读取权限。</li>
          <li>复制 hf_ 开头的 token，粘贴到这里，勾选启用本地 CL Tagger，再点下载模型到本机。</li>
        </ol>
        <p>模型页：huggingface.co/cella110n/cl_tagger_v2</p>
        <img :src="agreeShot" alt="模型页提示需要登录并同意许可" />
        <p>创建 token 时 Role 选 read。</p>
        <img :src="tokenShot" alt="Create a new access token，Role 为 read" />
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.panel { display: grid; gap: 10px; }
.card-lite {
  background: var(--bg0);
  border: 1px solid var(--bg3);
  border-radius: 8px;
  padding: 12px;
  display: grid;
  gap: 8px;
}
.gpus { margin: 0; padding-left: 18px; color: var(--muted); font-size: 13px; }
.check { display: flex; gap: 8px; align-items: center; }
.check.off { opacity: 0.6; }
.slide { display: grid; gap: 6px; font-size: 13px; color: var(--muted); }
.slide b { color: var(--heading); }
.progress {
  position: relative;
  min-height: 28px;
  padding: 6px 8px;
  border: 1px solid var(--bg3);
  border-radius: 6px;
  overflow: hidden;
  font-size: 12px;
  color: var(--muted);
}
.progress i {
  position: absolute;
  left: 0;
  bottom: 0;
  height: 3px;
  background: var(--heading);
}
.token-label { display: flex; align-items: center; gap: 6px; font-size: 12px; color: var(--muted); }
.help {
  width: 18px;
  height: 18px;
  padding: 0;
  border-radius: 50%;
  border: 1px solid var(--bg3);
  background: var(--bg0);
  color: var(--heading);
  font-size: 12px;
  line-height: 16px;
  font-weight: 700;
  flex: 0 0 auto;
}
.help:hover, .help:focus-visible { border-color: var(--heading); outline: none; }
.note { color: var(--heading); font-size: 13px; margin: 0; }
.path { word-break: break-all; user-select: text; }
code { font-size: 12px; }
</style>

<style>
.cl-token-help {
  position: fixed;
  z-index: 80;
  overflow: auto;
  padding: 14px 14px 12px;
  background: #191b31;
  border: 1px solid #3a3d5c;
  border-radius: 10px;
  box-shadow: 0 16px 40px rgba(0, 0, 0, 0.45);
  color: rgba(255, 255, 255, 0.82);
  font-size: 13px;
  line-height: 1.45;
}
.cl-token-help strong {
  display: block;
  margin-bottom: 8px;
  color: #f5f3c2;
  font-family: Eczar, "Times New Roman", serif;
  font-size: 16px;
}
.cl-token-help ol { margin: 0 0 8px; padding-left: 18px; }
.cl-token-help li + li { margin-top: 6px; }
.cl-token-help p { margin: 8px 0 6px; color: rgba(255, 255, 255, 0.55); font-size: 12px; word-break: break-all; }
.cl-token-help img {
  display: block;
  width: 100%;
  height: auto;
  border-radius: 6px;
  border: 1px solid #22253f;
}
</style>
