<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
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

const store = useAppStore();
const gpu = ref<GpuDetectResult | null>(null);
const status = ref<ClTaggerStatus | null>(null);
const hfDraft = ref("");
const message = ref("");
const busy = ref(false);
const downloadPct = ref(0);
const downloadMsg = ref("");
let unlisten: UnlistenFn | undefined;

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
  busy.value = true;
  downloadPct.value = 1;
  downloadMsg.value = "开始下载 gated 权重（约 1GB+，只保存在本机）…";
  message.value = downloadMsg.value;
  try {
    await persistHf();
    status.value = await clTaggerDownload();
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
      <label>Hugging Face token</label>
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
.note { color: var(--heading); font-size: 13px; margin: 0; }
.path { word-break: break-all; user-select: text; }
code { font-size: 12px; }
</style>
