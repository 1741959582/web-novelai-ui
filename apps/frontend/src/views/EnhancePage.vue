<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useAppStore } from "@/stores/app";
import {
  dlssnrEnhance,
  dlssnrInstall,
  dlssnrStatus,
  readImageDataUrl,
  type DlssStatus,
} from "@/api/tauri";
import type { HistoryItem } from "@/types/nai";

interface Preset {
  id: string;
  name: string;
  style: number;
  intensity: number;
  localStructure: number;
  localTone: number;
  detail: number;
  color: number;
  skin: number;
  autoMask: boolean;
}

const presets: Preset[] = [
  { id: "material-light", name: "质感光影", style: 2, intensity: 1, localStructure: 1.8, localTone: 1.4, detail: 1, color: 0.65, skin: 1, autoMask: true },
  { id: "color-keep", name: "保色增强", style: 2, intensity: 1, localStructure: 1.2, localTone: 1.8, detail: 1.1, color: 0.25, skin: 1.2, autoMask: true },
  { id: "rich", name: "浓郁", style: 2, intensity: 1, localStructure: 1.2, localTone: 1.8, detail: 1.1, color: 1, skin: 1.2, autoMask: true },
  { id: "soft", name: "柔和光影", style: 2, intensity: 1, localStructure: 1.2, localTone: 1.8, detail: 0.8, color: 1, skin: 1.2, autoMask: true },
  { id: "natural", name: "自然", style: 1, intensity: 1, localStructure: 1.2, localTone: 1.8, detail: 1.1, color: 1, skin: 1.2, autoMask: true },
  { id: "cinema-soft", name: "电影柔光", style: 2, intensity: 1, localStructure: 1.2, localTone: 1.3, detail: 0.9, color: 1, skin: 1.2, autoMask: true },
  { id: "crisp", name: "清晰", style: 2, intensity: 1, localStructure: 1.6, localTone: 1.8, detail: 1.2, color: 1, skin: 1.2, autoMask: true },
];

interface Job {
  id: string;
  path: string;
  prompt: string;
  negativePrompt: string;
  seed: number;
  model: string;
  groupId: string;
  originalUrl: string;
  enhancedUrl: string;
  width: number;
  height: number;
}

const store = useAppStore();
const jobs = ref<Job[]>([]);
const active = ref(0);
const presetId = ref(presets[0].id);
const scale = ref(2);
const status = ref<DlssStatus | null>(null);
const note = ref("");
const failed = ref(false);
const installing = ref(false);
const running = ref(false);
const follow = ref(localStorage.getItem("dlssnr-follow") === "1");
const stageEl = ref<HTMLElement | null>(null);
const worldEl = ref<HTMLElement | null>(null);
const frameEl = ref<HTMLElement | null>(null);
const clipEl = ref<HTMLElement | null>(null);
const dividerEl = ref<HTMLElement | null>(null);
const percentEl = ref<HTMLElement | null>(null);
let zoom = 1;
let panX = 0;
let panY = 0;
let split = 0.5;
let fitted = true;
let natW = 1;
let natH = 1;
let stageW = 1;
let stageH = 1;
let drag: "" | "pan" | "split" = "";
let lastX = 0;
let lastY = 0;
let observer: ResizeObserver | null = null;
let viewFrame = 0;

const current = computed(() => jobs.value[active.value] || null);
const preset = computed(() => presets.find((item) => item.id === presetId.value) || presets[0]);
const shownUrl = computed(() => current.value?.enhancedUrl || current.value?.originalUrl || "");

function applyView() {
  if (frameEl.value) {
    frameEl.value.style.width = `${natW}px`;
    frameEl.value.style.height = `${natH}px`;
  }
  if (worldEl.value) {
    worldEl.value.style.transform = `translate(calc(-50% + ${panX}px), calc(-50% + ${panY}px)) scale(${zoom})`;
  }
  if (clipEl.value) {
    clipEl.value.style.width = `${split * 100}%`;
    const image = clipEl.value.querySelector("img");
    if (image instanceof HTMLElement) {
      image.style.width = `${natW}px`;
      image.style.height = `${natH}px`;
    }
  }
  if (dividerEl.value) {
    const width = natW * zoom;
    const height = natH * zoom;
    dividerEl.value.style.left = `${stageW / 2 + panX - width / 2 + split * width}px`;
    dividerEl.value.style.top = `${stageH / 2 + panY - height / 2}px`;
    dividerEl.value.style.height = `${height}px`;
  }
  if (percentEl.value) percentEl.value.textContent = `${Math.round(zoom * 100)}%`;
}

function scheduleView() {
  if (viewFrame) return;
  viewFrame = requestAnimationFrame(() => {
    viewFrame = 0;
    applyView();
  });
}

function clampZoom(value: number) {
  return Math.min(8, Math.max(0.05, value));
}

function measure() {
  const box = stageEl.value?.getBoundingClientRect();
  if (!box) return;
  stageW = box.width;
  stageH = box.height;
  if (fitted) fit();
  else scheduleView();
}

function fit() {
  const box = stageEl.value?.getBoundingClientRect();
  if (!box || natW < 2 || natH < 2) return;
  const pad = 8;
  zoom = Math.min((box.width - pad) / natW, (box.height - pad) / natH);
  panX = 0;
  panY = 0;
  fitted = true;
  scheduleView();
}

function readSize(src: string) {
  const image = new Image();
  image.onload = () => {
    natW = image.naturalWidth || 1;
    natH = image.naturalHeight || 1;
    fit();
  };
  image.src = src;
}

watch(shownUrl, async (src) => {
  if (!src) return;
  readSize(src);
  await nextTick();
  applyView();
});

watch(follow, (value) => {
  localStorage.setItem("dlssnr-follow", value ? "1" : "0");
});

function setScale(input: HTMLInputElement) {
  const value = Number(input.value);
  const next = Number.isFinite(value) ? Math.min(4, Math.max(1, Math.round(value * 100) / 100)) : 2;
  scale.value = next;
  input.value = String(next);
}

function pointSplit(clientX: number) {
  const box = stageEl.value?.getBoundingClientRect();
  if (!box) return;
  const width = natW * zoom;
  const left = box.left + stageW / 2 + panX - width / 2;
  const next = Math.min(1, Math.max(0, (clientX - left) / Math.max(width, 1)));
  if (next === split) return;
  split = next;
  scheduleView();
}

function onPointerDown(event: PointerEvent) {
  const target = event.target as HTMLElement;
  drag = target.closest(".divider") ? "split" : "pan";
  lastX = event.clientX;
  lastY = event.clientY;
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  if (drag === "split") pointSplit(event.clientX);
}

function onPointerMove(event: PointerEvent) {
  if (follow.value && drag !== "pan") pointSplit(event.clientX);
  if (!drag) return;
  if (drag === "split") {
    pointSplit(event.clientX);
    return;
  }
  panX += event.clientX - lastX;
  panY += event.clientY - lastY;
  lastX = event.clientX;
  lastY = event.clientY;
  fitted = false;
  scheduleView();
}

function onPointerUp() {
  drag = "";
}

function onWheel(event: WheelEvent) {
  const box = stageEl.value?.getBoundingClientRect();
  if (!box) return;
  const next = clampZoom(zoom * (event.deltaY < 0 ? 1.12 : 1 / 1.12));
  const cx = event.clientX - box.left - box.width / 2;
  const cy = event.clientY - box.top - box.height / 2;
  const ratio = next / zoom;
  panX = cx - (cx - panX) * ratio;
  panY = cy - (cy - panY) * ratio;
  zoom = next;
  fitted = false;
  scheduleView();
}

function zoomBy(factor: number) {
  zoom = clampZoom(zoom * factor);
  fitted = false;
  scheduleView();
}

function actualSize() {
  zoom = 1;
  panX = 0;
  panY = 0;
  fitted = false;
  scheduleView();
}

function explainDlss(error: unknown) {
  const text = error instanceof Error ? error.message : String(error);
  if (text.includes("too old") || text.includes("616.56")) {
    return "增强没有生效。这台电脑的 NVIDIA 驱动低于 616.56，DLSS 神经渲染拒绝运行，所以左右两边都还是原图。请到 NVIDIA 官网安装 616.56 或更新的驱动后再试。";
  }
  return text;
}

async function loadQueue() {
  let ids: string[] = [];
  try {
    const raw = JSON.parse(sessionStorage.getItem("nai-dlssnr-ids") || "[]");
    if (Array.isArray(raw)) ids = raw.filter((id) => typeof id === "string");
  } catch {
    ids = [];
  }
  const chosen = ids
    .map((id) => store.history.find((item) => item.id === id))
    .filter((item): item is HistoryItem => !!item)
    .slice(0, 24);
  jobs.value = chosen.map((item) => ({
    id: item.id,
    path: item.path,
    prompt: item.prompt,
    negativePrompt: item.negativePrompt,
    seed: item.seed,
    model: item.model,
    groupId: item.groupId || "",
    originalUrl: "",
    enhancedUrl: "",
    width: item.width,
    height: item.height,
  }));
  active.value = 0;
  if (!jobs.value.length) {
    note.value = "先在生成记录里勾选图片，再点 DLSSNR 增强。";
    return;
  }
  note.value = jobs.value.length > 1 ? `已带入 ${jobs.value.length} 张，点一张切换对比。` : "";
  await show(0);
}

async function show(index: number) {
  active.value = index;
  split = 0.5;
  scheduleView();
  const job = jobs.value[index];
  if (!job || job.originalUrl) return;
  try {
    job.originalUrl = await readImageDataUrl(job.path);
  } catch (error) {
    note.value = error instanceof Error ? error.message : "这张图读不出来";
  }
}

async function refreshStatus() {
  try {
    status.value = await dlssnrStatus();
  } catch (error) {
    note.value = error instanceof Error ? error.message : String(error);
  }
}

async function install() {
  if (installing.value) return;
  installing.value = true;
  note.value = "正在下载并校验运行库…";
  try {
    status.value = await dlssnrInstall();
    note.value = status.value.message;
  } catch (error) {
    note.value = explainDlss(error);
  } finally {
    installing.value = false;
  }
}

async function enhanceOne(index: number) {
  const job = jobs.value[index];
  const options = preset.value;
  if (!job) return;
  const item = await dlssnrEnhance({
    sourcePath: job.path,
    scale: scale.value,
    style: options.style,
    intensity: options.intensity,
    localStructure: options.localStructure,
    localTone: options.localTone,
    detail: options.detail,
    color: options.color,
    skin: options.skin,
    autoMask: options.autoMask,
    prompt: job.prompt,
    negativePrompt: job.negativePrompt,
    seed: job.seed,
    model: job.model,
    groupId: job.groupId,
  });
  store.history = [item, ...store.history.filter((entry) => entry.id !== item.id)];
  job.enhancedUrl = await readImageDataUrl(item.path);
  job.width = item.width;
  job.height = item.height;
}

async function enhanceCurrent() {
  if (running.value || !current.value) return;
  if (!status.value?.installed) {
    note.value = status.value?.message || "先安装运行库。";
    return;
  }
  running.value = true;
  failed.value = false;
  note.value = "正在本机增强，原图不会被覆盖。";
  try {
    await enhanceOne(active.value);
    note.value = "增强结果已另存，并写进生成记录。拖动分割线对比。";
  } catch (error) {
    failed.value = true;
    note.value = explainDlss(error);
  } finally {
    running.value = false;
  }
}

async function enhanceAll() {
  if (running.value || !jobs.value.length) return;
  if (!status.value?.installed) {
    note.value = status.value?.message || "先安装运行库。";
    return;
  }
  running.value = true;
  try {
    for (let index = 0; index < jobs.value.length; index += 1) {
      active.value = index;
      note.value = `正在增强 ${index + 1} / ${jobs.value.length}`;
      await show(index);
      await enhanceOne(index);
    }
    note.value = `已增强 ${jobs.value.length} 张，新图写在记录最前，原图还在。`;
  } catch (error) {
    failed.value = true;
    note.value = explainDlss(error);
  } finally {
    running.value = false;
  }
}

onMounted(async () => {
  await refreshStatus();
  await loadQueue();
  await nextTick();
  measure();
  if (stageEl.value) {
    observer = new ResizeObserver(() => measure());
    observer.observe(stageEl.value);
  }
});

onBeforeUnmount(() => {
  observer?.disconnect();
  if (viewFrame) cancelAnimationFrame(viewFrame);
});
</script>

<template>
  <section class="page">
    <header>
      <h2>DLSSNR 图像增强</h2>
      <div class="tools">
        <label>风格
          <select v-model="presetId" :disabled="running">
            <option v-for="item in presets" :key="item.id" :value="item.id">{{ item.name }}</option>
          </select>
        </label>
        <label>倍率
          <input :value="scale" type="number" min="1" max="4" step="0.25" :disabled="running" @change="setScale($event.target as HTMLInputElement)" />
        </label>
        <button type="button" class="btn" :disabled="installing || running || status?.installed" @click="install">
          {{ installing ? "正在安装…" : status?.installed ? `运行库 ${status.tag}` : "安装运行库" }}
        </button>
        <button type="button" class="btn primary" :disabled="!current || running || !status?.installed" @click="enhanceCurrent">
          {{ running ? "正在增强…" : "增强当前" }}
        </button>
        <button v-if="jobs.length > 1" type="button" class="btn" :disabled="running || !status?.installed" @click="enhanceAll">全部增强</button>
      </div>
    </header>

    <div class="body">
      <aside v-if="jobs.length" class="queue">
        <button
          v-for="(job, index) in jobs"
          :key="job.id"
          type="button"
          :class="{ on: index === active }"
          @click="show(index)"
        >
          <img v-if="job.originalUrl || job.enhancedUrl" :src="job.enhancedUrl || job.originalUrl" alt="" />
          <span v-else>{{ index + 1 }}</span>
        </button>
      </aside>

      <div class="stage-wrap">
        <div class="labels"><span>增强结果</span><span>原图</span></div>
        <div
          ref="stageEl"
          class="stage"
          @pointerdown="onPointerDown"
          @pointermove="onPointerMove"
          @pointerup="onPointerUp"
          @pointercancel="onPointerUp"
          @wheel.prevent="onWheel"
        >
          <div v-if="current && shownUrl" ref="worldEl" class="world">
            <div ref="frameEl" class="frame">
              <img class="original" :src="current.originalUrl || shownUrl" alt="原图" draggable="false" />
              <div ref="clipEl" class="clip">
                <img class="enhanced" :src="current.enhancedUrl || current.originalUrl" alt="增强结果" draggable="false" />
              </div>
            </div>
          </div>
          <p v-else class="empty">{{ note || "没有待增强的图片" }}</p>
          <p v-if="failed && note" class="banner">{{ note }}</p>
          <div v-if="current && shownUrl" ref="dividerEl" class="divider">
            <span class="grip" aria-hidden="true"><i /><i /><i /><i /><i /><i /></span>
          </div>
        </div>

        <div class="bar">
          <div class="zoom">
            <button type="button" title="缩小" @click="zoomBy(1 / 1.25)">−</button>
            <span ref="percentEl">100%</span>
            <button type="button" title="放大" @click="zoomBy(1.25)">+</button>
            <button type="button" @click="actualSize">100%</button>
            <button type="button" @click="fit">适应窗口</button>
          </div>
          <button type="button" class="follow" @click="follow = !follow">跟随鼠标
            <span class="switch" :class="{ on: follow }" />
          </button>
        </div>
        <p class="caption">完整分辨率对比，默认适应窗口。点击 100% 查看原尺寸像素；拖动图像查看局部，拖动分割线对比。</p>
        <p v-if="note" class="note">{{ note }}</p>
      </div>
    </div>
  </section>
</template>

<style scoped>
.page { display: flex; flex-direction: column; min-height: 0; background: var(--bg0); }
header { display: flex; align-items: center; justify-content: space-between; gap: 16px; padding: 12px 16px 8px; }
h2 { margin: 0; font-size: 20px; }
.tools { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; justify-content: flex-end; }
.tools label { display: flex; align-items: center; gap: 6px; color: var(--muted); font-size: 13px; }
select, .tools input { height: 28px; padding: 0 8px; border: 1px solid var(--bg3); border-radius: 6px; background: var(--bg1); }
.tools input { width: 72px; text-align: right; }
.btn { height: 28px; padding: 0 10px; border: 1px solid var(--bg3); border-radius: 6px; background: var(--bg2); }
.btn.primary { background: var(--heading); color: var(--bg0); border-color: var(--heading); }
.btn:disabled { opacity: 0.45; }
.body { flex: 1; min-height: 0; display: flex; gap: 10px; padding: 0 12px 12px; }
.queue { width: 72px; overflow: auto; display: flex; flex-direction: column; gap: 6px; }
.queue button { height: 72px; padding: 0; border: 1px solid var(--bg3); border-radius: 8px; background: var(--bg1); overflow: hidden; }
.queue button.on { border-color: var(--heading); }
.queue img { width: 100%; height: 100%; object-fit: cover; }
.stage-wrap { flex: 1; min-width: 0; display: flex; flex-direction: column; background: var(--bg1); border: 1px solid var(--bg3); border-radius: 12px; overflow: hidden; }
.labels { display: flex; justify-content: space-between; padding: 8px 14px 0; color: rgba(255,255,255,.82); font-size: 13px; }
.stage { position: relative; flex: 1; min-height: 0; overflow: hidden; cursor: grab; touch-action: none; }
.stage:active { cursor: grabbing; }
.world { position: absolute; left: 50%; top: 50%; transform-origin: center; will-change: transform; }
.frame { position: relative; }
.frame img { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: fill; pointer-events: none; user-select: none; }
.clip { position: absolute; top: 0; left: 0; height: 100%; width: 50%; overflow: hidden; }
.clip img { max-width: none; }
.empty { margin: auto; position: absolute; inset: 0; display: grid; place-items: center; color: var(--muted); }
.divider { position: absolute; z-index: 3; width: 28px; transform: translateX(-50%); cursor: ew-resize; }
.divider::before { content: ""; position: absolute; left: 50%; top: 0; bottom: 0; width: 1px; background: rgba(255,255,255,.9); }
.grip { position: absolute; left: 50%; top: 50%; width: 28px; height: 42px; transform: translate(-50%, -50%); border-radius: 14px; background: #1b1d22; display: grid; grid-template-columns: 4px 4px; gap: 3px; place-content: center; }
.grip i { width: 4px; height: 4px; border-radius: 50%; background: rgba(255,255,255,.8); }
.bar { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 8px 12px; background: #17181c; }
.zoom { display: flex; align-items: center; gap: 6px; }
.zoom button, .zoom span { min-width: 28px; height: 28px; border: 0; background: transparent; color: #fff; }
.zoom span { display: grid; place-items: center; font-variant-numeric: tabular-nums; }
.follow { display: flex; align-items: center; gap: 8px; border: 0; background: transparent; color: rgba(255,255,255,.8); font-size: 13px; font-weight: 500; }
.switch { width: 36px; height: 20px; border-radius: 10px; background: #3a3d44; position: relative; }
.switch::after { content: ""; position: absolute; top: 2px; left: 2px; width: 16px; height: 16px; border-radius: 50%; background: #fff; transition: transform .15s ease; }
.switch.on { background: #5b8cff; }
.switch.on::after { transform: translateX(16px); }
.caption, .note { margin: 0; padding: 6px 14px 8px; color: var(--muted); font-size: 12px; }
.note { color: var(--heading); padding-top: 0; }
.banner { position: absolute; left: 12px; right: 12px; bottom: 12px; z-index: 2; margin: 0; padding: 8px 10px; border-radius: 8px; background: rgba(14, 15, 33, .92); color: var(--heading); font-size: 13px; pointer-events: none; }
</style>
