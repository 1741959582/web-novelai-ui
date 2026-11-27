<script setup lang="ts">
import { computed, getCurrentInstance, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { useRoute } from "vue-router";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { NAI_MODELS, NAI_SAMPLERS, isV4Plus, maxCharacterPrompts, supportsNAIPreciseReference, supportsNAIVibeTransfer } from "@/types/nai";
import { useAppStore } from "@/stores/app";
import { useMaterialLibrary } from "@/stores/materialLibrary";
import { promptBlockName } from "@/utils/artistString";
import { fetchRemoteImage } from "@/api/tauri";
import { quoteAnlas } from "@/utils/anlas";
import { applyResolution, inferResolution, type Orientation, type ResolutionFamily } from "@/utils/resolution";
import HistoryRail from "@/components/HistoryRail.vue";
import NaiIcon from "@/components/NaiIcon.vue";
import StarterGallery from "@/components/StarterGallery.vue";
import ImportImageDialog from "@/components/ImportImageDialog.vue";
import InpaintEditor from "@/components/InpaintEditor.vue";
import PreviewToolbar from "@/components/PreviewToolbar.vue";
import CharacterPositionOverlay from "@/components/CharacterPositionOverlay.vue";
import CompositionGuide from "@/components/CompositionGuide.vue";
import OpusUsageBar from "@/components/OpusUsageBar.vue";
import ReferencePanel from "@/components/ReferencePanel.vue";
import PromptField from "@/components/PromptField.vue";
import PromptToolbox from "@/components/PromptToolbox.vue";
import DanbooruBubble from "@/components/DanbooruBubble.vue";
import NaiAgentMark from "@/components/NaiAgentMark.vue";
import NaiAgentPanel from "@/components/NaiAgentPanel.vue";
import { loadAutoComplete } from "@/utils/promptTools";
import { appendPromptChunk } from "@/utils/textUtils";

const store = useAppStore();
const materials = useMaterialLibrary();
const route = useRoute();
const aiOpen = ref(false);
const NOISE_SCHEDULES = [
  { value: "native", label: "Native" },
  { value: "karras", label: "Karras" },
  { value: "exponential", label: "Exponential" },
  { value: "polyexponential", label: "Polyexponential" },
];
const dropping = ref(false);
const guideOpen = ref(false);
const guideBtn = ref<HTMLButtonElement | null>(null);
const guidePopStyle = ref<Record<string, string>>({});
const previewCollapsed = ref(false);
const promptTab = ref<"prompt" | "uc">("prompt");
const agentOpen = ref(false);
const agentRight = ref(localStorage.getItem("nai-agent-dock") === "right");
function setAgentDock(right: boolean) {
  agentRight.value = right;
  localStorage.setItem("nai-agent-dock", right ? "right" : "here");
}
const charMenu = ref(false);
const pageVm = getCurrentInstance() as { job?: () => void; update?: () => void } | null;
function paintAgent() {
  const run = pageVm?.job || pageVm?.update;
  if (typeof run === "function") run();
}
function toggleAgent() {
  agentOpen.value = !agentOpen.value;
  paintAgent();
}
function closeAgent() {
  agentOpen.value = false;
  paintAgent();
}
const charTab = ref<Record<string, "prompt" | "uc">>({});
const autoComplete = ref(loadAutoComplete());
watch(
  () => store.previewUrl,
  (next, prev) => {
    if (next && next !== prev) previewCollapsed.value = store.settings.stealthMode;
  },
);
watch(
  () => store.settings.stealthMode,
  (on) => {
    if (on && !store.positionEditorOpen) previewCollapsed.value = true;
  },
  { immediate: true },
);
const tagTargets = computed(() => [
  { id: "positive", label: "基础提示词" },
  { id: "negative", label: "负面" },
  ...store.characters.flatMap((character, index) => [
    { id: `char:${character.id}:prompt`, label: `角色 ${index + 1} 正面` },
    { id: `char:${character.id}:uc`, label: `角色 ${index + 1} 负面` },
  ]),
]);

function placeTag(id: string, tag: string) {
  if (id === "positive") {
    store.params.positivePrompt = appendPromptChunk(store.params.positivePrompt, tag);
    return;
  }
  if (id === "negative") {
    store.params.negativePrompt = appendPromptChunk(store.params.negativePrompt, tag);
    return;
  }
  const match = /^char:(.+):(prompt|uc)$/.exec(id);
  if (!match) return;
  const character = store.characters.find((item) => item.id === match[1]);
  if (!character) return;
  if (match[2] === "prompt") store.updateCharacter(character.id, { prompt: appendPromptChunk(character.prompt, tag) });
  else store.updateCharacter(character.id, { negativePrompt: appendPromptChunk(character.negativePrompt, tag) });
}

const promptValue = computed({
  get: () => (promptTab.value === "prompt" ? store.params.positivePrompt : store.params.negativePrompt),
  set: (next: string) => {
    if (promptTab.value === "prompt") store.params.positivePrompt = next;
    else store.params.negativePrompt = next;
  },
});

const family = computed({
  get: () => inferResolution(store.params.width, store.params.height).family,
  set: (next: ResolutionFamily) => {
    const { orientation } = inferResolution(store.params.width, store.params.height);
    const size = applyResolution(next, orientation);
    store.params.width = size.width;
    store.params.height = size.height;
  },
});

const orientation = computed({
  get: () => inferResolution(store.params.width, store.params.height).orientation,
  set: (next: Orientation) => {
    const size = applyResolution(family.value, next);
    store.params.width = size.width;
    store.params.height = size.height;
  },
});

const inpaintOn = computed(() => store.paintMode && Boolean(store.inpaintMask));
const actionKind = computed(() => (inpaintOn.value ? "infill" : store.i2iImage ? "img2img" : "generate"));
const anlas = computed(() =>
  quoteAnlas({
    params: store.params,
    account: store.account,
    batchCount: store.batchCount,
    action: actionKind.value,
    strength: inpaintOn.value ? 1 : store.i2iStrength,
    vibeCount: supportsNAIVibeTransfer(store.params.model) ? store.vibeImages.filter((v) => v.enabled).length : 0,
    preciseCount: supportsNAIPreciseReference(store.params.model) ? store.preciseReferences.filter((p) => p.enabled).length : 0,
  }),
);
const generateLabel = computed(() => {
  const n = store.batchCount;
  if (inpaintOn.value) return n === 1 ? "重绘 1 张图片" : `重绘 ${n} 张图片`;
  return n === 1 ? "生成 1 张图片" : `生成 ${n} 张图片`;
});
const displayProgress = computed(() => Math.min(1, Math.max(0, store.genProgress)));
const streamTitle = computed(() => {
  if (store.streamReplaying) return "回放预览";
  if (store.genPhase === "saving") return "正在保存";
  if (store.genPreview && store.settings.streamPreviewEnabled) return "流式生成中";
  if (store.genPhase === "waiting") return "正在连接";
  return "正在生成";
});
const canCharacters = computed(() => isV4Plus(store.params.model));
const maxChars = computed(() => maxCharacterPrompts(store.params.model));
const samplerLabel = computed(
  () => NAI_SAMPLERS.find((s) => s.value === store.params.sampler)?.short || store.params.sampler,
);
const cfgText = computed(() => store.params.cfgScale.toFixed(1));
const rescaleText = computed(() => store.params.cfgRescale.toFixed(2));

watch(
  () => store.params.model,
  () => {
    const max = maxCharacterPrompts(store.params.model);
    if (store.characters.length > max) store.characters.splice(max);
  },
);

function seedDisplay() {
  return store.params.seedMode === "fixed" && store.params.seed > 0 ? String(store.params.seed) : "";
}

function onSeedInput(ev: Event) {
  const raw = (ev.target as HTMLInputElement).value.trim();
  if (!raw) {
    store.clearSeed();
    return;
  }
  const n = Number(raw);
  if (!Number.isFinite(n) || n <= 0) {
    store.clearSeed();
    return;
  }
  store.params.seed = Math.floor(n);
  store.params.seedMode = "fixed";
}

function applyRefFile(file: File, kind: "i2i" | "vibe" | "precise" | "inset" = "i2i") {
  void fileToDataUrl(file).then((url) => store.useImageAs(kind, url));
}

function isHttpUrl(value: string) {
  return /^https?:\/\//i.test(value.trim());
}

function isLocalPath(value: string) {
  const path = value.trim();
  return /^(?:\\\\\?\\)?([a-zA-Z]:[\\/]|\\\\|\/)/.test(path) && !isHttpUrl(path);
}

function fileUrlToPath(value: string) {
  const raw = value.trim();
  if (!/^file:/i.test(raw)) return "";
  try {
    const url = new URL(raw);
    let path = decodeURIComponent(url.pathname);
    if (/^\/[a-zA-Z]:/.test(path)) path = path.slice(1);
    return path.replace(/\//g, "\\");
  } catch {
    return raw.replace(/^file:\/+/i, "").replace(/\//g, "\\");
  }
}

function safeTransferText(dt: DataTransfer | null | undefined, type: string) {
  try {
    return dt?.getData(type) || "";
  } catch {
    return "";
  }
}

function pathFromTransferText(value: string) {
  const item = value.trim();
  if (!item || item.startsWith("#")) return "";
  if (isHttpUrl(item)) return "";
  if (/^file:/i.test(item)) return fileUrlToPath(item);
  if (isLocalPath(item)) return item;
  return "";
}

function imageUrlFromTransfer(dt: DataTransfer | null | undefined) {
  if (!dt) return "";
  const download = safeTransferText(dt, "DownloadURL");
  if (download) {
    const parts = download.split(":");
    const href = parts.slice(2).join(":");
    if (isHttpUrl(href)) return href.trim();
  }
  const uri = `${safeTransferText(dt, "text/uri-list")}\n${safeTransferText(dt, "text/plain")}`;
  for (const line of uri.split(/\r?\n/)) {
    const item = line.trim();
    if (item && !item.startsWith("#") && isHttpUrl(item)) return item;
  }
  const html = safeTransferText(dt, "text/html");
  const img = html.match(/<img[^>]+src=["']([^"']+)/i)?.[1];
  if (img?.startsWith("data:image/") && img.length < 8_000_000) return img;
  if (img && isHttpUrl(img)) return img;
  return "";
}

function localPathFromTransfer(dt: DataTransfer | null | undefined) {
  if (!dt) return "";
  const uri = `${safeTransferText(dt, "text/uri-list")}\n${safeTransferText(dt, "text/plain")}`;
  for (const line of uri.split(/\r?\n/)) {
    const path = pathFromTransferText(line);
    if (path) return path;
  }
  const html = safeTransferText(dt, "text/html");
  const img = html.match(/<img[^>]+src=["']([^"']+)/i)?.[1];
  return img ? pathFromTransferText(img) : "";
}

function hasImageDrag(ev: DragEvent) {
  const types = [...(ev.dataTransfer?.types ?? [])];
  return types.includes("Files") || types.includes("text/uri-list") || types.includes("text/html") || types.includes("text/plain");
}

function onPickRef(file: File, kind?: "i2i" | "vibe" | "precise" | "inset") {
  if (kind) {
    applyRefFile(file, kind);
    return;
  }
  void store.openImportFromFile(file);
}

function dropKindFromTarget(target: EventTarget | null): "i2i" | "vibe" | "precise" | "inset" | undefined {
  const el = target instanceof Element ? target.closest("[data-ref-drop]") : null;
  const kind = el?.getAttribute("data-ref-drop");
  if (kind === "i2i" || kind === "vibe" || kind === "precise" || kind === "inset") return kind;
}

function refKindAtPoint(x: number, y: number) {
  return dropKindFromTarget(document.elementFromPoint(x, y));
}

function fileToDataUrl(file: Blob) {
  return new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = reject;
    reader.readAsDataURL(file);
  });
}

function isImageFile(file: File) {
  if (file.type.startsWith("image/")) return true;
  return /\.(png|jpe?g|webp|gif|bmp)$/i.test(file.name);
}

function imageFromEvent(ev: DragEvent | ClipboardEvent) {
  if ("clipboardData" in ev && ev.clipboardData) {
    const fromFiles = [...ev.clipboardData.files].find(isImageFile);
    if (fromFiles) return fromFiles;
    for (const item of ev.clipboardData.items) {
      if (item.kind !== "file") continue;
      const file = item.getAsFile();
      if (file && isImageFile(file)) return file;
    }
    return undefined;
  }
  if ("dataTransfer" in ev && ev.dataTransfer) {
    for (const item of ev.dataTransfer.items) {
      if (item.kind !== "file") continue;
      const file = item.getAsFile();
      if (file && isImageFile(file)) return file;
    }
    return [...ev.dataTransfer.files].find(isImageFile);
  }
  return undefined;
}

let dragDepth = 0;
let tauriDropHandled = false;
let importing = false;

function waitForTauriDrop(ms = 280) {
  return new Promise<void>((resolve) => {
    if (tauriDropHandled) {
      resolve();
      return;
    }
    const started = Date.now();
    const timer = window.setInterval(() => {
      if (tauriDropHandled || Date.now() - started >= ms) {
        window.clearInterval(timer);
        resolve();
      }
    }, 20);
  });
}

async function importDropped(kind: "i2i" | "vibe" | "precise" | "inset" | undefined, file?: File, url?: string, localPath?: string) {
  if (importing) return;
  importing = true;
  store.status = "正在导入图片…";
  try {
    if (localPath) {
      if (kind) await store.usePathAs(kind, localPath);
      else await store.openImportFromPath(localPath);
      return;
    }
    if (file) {
      if (kind) applyRefFile(file, kind);
      else await store.openImportFromFile(file);
      return;
    }
    if (!url) return;
    if (kind) {
      store.useImageAs(kind, url.startsWith("data:image/") ? url : await fetchRemoteImage(url));
      return;
    }
    if (url.startsWith("data:image/")) {
      await store.openImportFromFile(await (await fetch(url)).blob());
      return;
    }
    await store.openImportFromUrl(url);
  } catch (e) {
    store.status = e instanceof Error ? e.message : String(e);
  } finally {
    importing = false;
  }
}

function onWinDragEnter(ev: DragEvent) {
  if (!hasImageDrag(ev)) return;
  ev.preventDefault();
  dragDepth += 1;
  dropping.value = !dropKindFromTarget(ev.target);
}
function onWinDragOver(ev: DragEvent) {
  if (!hasImageDrag(ev)) return;
  ev.preventDefault();
  if (ev.dataTransfer) ev.dataTransfer.dropEffect = "copy";
  dropping.value = !dropKindFromTarget(ev.target);
}
function onWinDragLeave(ev: DragEvent) {
  if (!hasImageDrag(ev)) return;
  dragDepth = Math.max(0, dragDepth - 1);
  if (dragDepth === 0) dropping.value = false;
}
function onWinDrop(ev: DragEvent) {
  ev.preventDefault();
  ev.stopPropagation();
  dragDepth = 0;
  dropping.value = false;
  const kind = dropKindFromTarget(ev.target);
  const types = [...(ev.dataTransfer?.types ?? [])];
  const nativeFiles = types.includes("Files");
  const url = imageUrlFromTransfer(ev.dataTransfer);
  const localPath = localPathFromTransfer(ev.dataTransfer);
  void (async () => {
    // QQ / Explorer drops freeze WebView2 if we call getAsFile() or .files.
    // Wait for Tauri's native path event instead.
    if (nativeFiles) {
      await waitForTauriDrop();
      if (tauriDropHandled) return;
      if (localPath) {
        await importDropped(kind, undefined, undefined, localPath);
        return;
      }
      if (url) {
        await importDropped(kind, undefined, url);
        return;
      }
      store.status = "拖入的不是图片。可从网页拖图片，或拖本地文件。";
      return;
    }
    if (localPath) {
      await importDropped(kind, undefined, undefined, localPath);
      return;
    }
    if (url) {
      await importDropped(kind, undefined, url);
      return;
    }
    store.status = "拖入的不是图片。可从网页拖图片，或拖本地文件。";
  })();
}

function onPaste(ev: ClipboardEvent) {
  const file = imageFromEvent(ev);
  if (!file) return;
  const target = ev.target instanceof Element ? ev.target : null;
  if (agentOpen.value || target?.closest(".agent-dock")) {
    ev.preventDefault();
    window.dispatchEvent(new CustomEvent("nai-agent-paste-image", { detail: file }));
    return;
  }
  ev.preventDefault();
  void store.openImportFromFile(file);
}

let unlistenTauri: (() => void) | undefined;
function placeGuidePop() {
  const btn = guideBtn.value;
  if (!btn) return;
  const rect = btn.getBoundingClientRect();
  const width = 280;
  const gap = 8;
  const left = rect.right + gap;
  if (left + width <= window.innerWidth - 8) {
    guidePopStyle.value = {
      left: `${left}px`,
      bottom: `${Math.max(8, window.innerHeight - rect.bottom)}px`,
    };
    return;
  }
  guidePopStyle.value = {
    right: `${Math.max(8, window.innerWidth - rect.right)}px`,
    bottom: `${window.innerHeight - rect.top + gap}px`,
  };
}

function toggleGuide() {
  guideOpen.value = !guideOpen.value;
  if (guideOpen.value) void nextTick(placeGuidePop);
}

function closeCharMenu(ev: Event) {
  const el = ev.target as HTMLElement | null;
  if (!el?.closest(".add-wrap")) charMenu.value = false;
  if (!el?.closest(".model-pick")) modelOpen.value = false;
  if (!el?.closest(".guide-btn") && !el?.closest(".guide-pop")) guideOpen.value = false;
}

function addCharacterKind(prompt: string) {
  if (store.addCharacter(prompt)) charMenu.value = false;
}

function clearAllCharacters() {
  store.clearCharacters();
  charMenu.value = false;
}

function onPosKey(ev: KeyboardEvent) {
  if (ev.key === "Escape" && store.positionEditorOpen) {
    ev.preventDefault();
    store.finishPositionEditor();
  }
}

onMounted(() => {
  window.addEventListener("pointerdown", closeCharMenu);
  window.addEventListener("keydown", onPosKey);
  window.addEventListener("paste", onPaste);
  window.addEventListener("dragenter", onWinDragEnter, true);
  window.addEventListener("dragover", onWinDragOver, true);
  window.addEventListener("dragleave", onWinDragLeave, true);
  window.addEventListener("drop", onWinDrop, true);
  try {
    void getCurrentWebview()
      .onDragDropEvent((event) => {
        const kind = event.payload.type;
        if (kind === "enter" || kind === "over") {
          dropping.value = true;
          return;
        }
        if (kind === "leave") {
          dropping.value = false;
          return;
        }
        if (kind === "drop") {
          dropping.value = false;
          const paths = event.payload.paths ?? [];
          const raw = paths.find((item) => {
            const value = item.trim();
            return Boolean(value) && (isHttpUrl(value) || isLocalPath(value) || /^file:/i.test(value));
          });
          if (!raw) return;
          tauriDropHandled = true;
          window.setTimeout(() => {
            tauriDropHandled = false;
          }, 800);
          const path = /^file:/i.test(raw) ? fileUrlToPath(raw) || raw : raw;
          const pos = "position" in event.payload ? event.payload.position : undefined;
          const dpr = window.devicePixelRatio || 1;
          const refKind = pos ? refKindAtPoint(pos.x / dpr, pos.y / dpr) : undefined;
          if (isHttpUrl(path)) {
            void importDropped(refKind, undefined, path);
            return;
          }
          void importDropped(refKind, undefined, undefined, path);
        }
      })
      .then((fn) => {
        unlistenTauri = fn;
      })
      .catch(() => {
        /* browser preview without Tauri */
      });
  } catch {
    /* getCurrentWebview throws synchronously outside the desktop webview */
  }
});
onUnmounted(() => {
  window.removeEventListener("pointerdown", closeCharMenu);
  window.removeEventListener("keydown", onPosKey);
  window.removeEventListener("paste", onPaste);
  window.removeEventListener("dragenter", onWinDragEnter, true);
  window.removeEventListener("dragover", onWinDragOver, true);
  window.removeEventListener("dragleave", onWinDragLeave, true);
  window.removeEventListener("drop", onWinDrop, true);
  unlistenTauri?.();
});

function bumpGuide(axis: "cols" | "rows", delta: number) {
  const key = axis === "cols" ? "overlayCols" : "overlayRows";
  store[key] = Math.max(2, Math.min(12, store[key] + delta));
  store.overlayGuide = "grid";
}

function runGenerate() {
  void store.generate(store.i2iImage || store.inpaintMask ? "img2img" : "txt2img");
}

watch(
  () => route.path,
  (path) => {
    if (path === "/inpaint") void store.startInpaint();
  },
  { immediate: true },
);

const showModelMode = computed(() => isV4Plus(store.params.model));
const modelOpen = ref(false);

function saveArtistBlock() {
  const positive = store.params.positivePrompt.trim();
  const negative = store.params.negativePrompt.trim();
  if (!positive && !negative) {
    store.status = "基础提示词和负面都是空的";
    return;
  }
  if (materials.blocks.some((block) => block.positive === positive && block.negative === negative)) {
    store.status = "这组基础提示词和负面已经在块库里";
    return;
  }
  const block = materials.addBlock({ name: promptBlockName(positive), positive, negative });
  store.status = `已把基础提示词和负面一起存成「${block.name}」`;
}

function toggleModelMode() {
  store.params.modelMode = store.params.modelMode === "furry" ? "anime" : "furry";
}

function modelShort(value: string) {
  if (value.endsWith("4-5-full")) return "V4.5 Full";
  if (value.endsWith("4-5-curated")) return "V4.5 Curated";
  if (value.endsWith("5-full")) return "V5 Full";
  if (value.endsWith("5-curated")) return "V5 Curated";
  if (value.endsWith("4-full")) return "V4 Full";
  if (value.endsWith("4-curated")) return "V4 Curated";
  if (value.includes("furry")) return "Furry V3";
  return "V3";
}

function toggleSeedPin() {
  if (store.params.seedMode === "fixed" && store.params.seed > 0) store.clearSeed();
  else store.rollSeed();
}
</script>

<template>
  <div class="page" :class="{ 'agent-right': agentOpen && agentRight }">
    <div v-if="dropping" class="drop-mask">
      <div class="drop-card">
        <NaiIcon name="upload" :size="40" />
      </div>
    </div>
    <section class="left">
      <div class="left-scroll">
      <div class="model-row">
        <div class="model-capsule">
          <span>Model</span>
          <div class="model-pick">
            <button type="button" class="model" @click.stop="modelOpen = !modelOpen">
              {{ modelShort(store.params.model) }}
              <NaiIcon name="down" :size="12" />
            </button>
            <div v-if="modelOpen" class="model-menu">
              <button
                v-for="m in NAI_MODELS"
                :key="m.value"
                type="button"
                :class="{ on: store.params.model === m.value }"
                @click="store.params.model = m.value; modelOpen = false"
              >{{ modelShort(m.value) }}</button>
            </div>
          </div>
          <template v-if="showModelMode">
            <i class="split"></i>
            <button type="button" class="mode" :title="store.params.modelMode === 'furry' ? 'Furry' : 'Anime'" @click="toggleModelMode">
              <NaiIcon :name="store.params.modelMode === 'furry' ? 'furry' : 'sparkle'" :size="14" />
              {{ store.params.modelMode === "furry" ? "Furry" : "Anime" }}
            </button>
          </template>
        </div>
      </div>

      <section class="block">
        <div class="text-tabs">
          <button type="button" :class="{ on: promptTab === 'prompt' }" @click="promptTab = 'prompt'">基础提示词</button>
          <button type="button" :class="{ on: promptTab === 'uc' }" @click="promptTab = 'uc'">负面</button>
          <NaiAgentMark class="agent-slot" :expanded="agentOpen" @toggle="toggleAgent" />
        </div>
        <PromptField
          v-model="promptValue"
          :enabled="autoComplete"
          :targets="tagTargets"
          :target-id="promptTab === 'prompt' ? 'positive' : 'negative'"
          :placeholder="promptTab === 'prompt' ? 'Enter your prompt here...' : 'lowres, worst quality...'"
          @place="placeTag"
        >
          <template v-if="promptTab === 'prompt'" #tools>
            <DanbooruBubble
              :text="store.params.positivePrompt"
              @apply="store.params.positivePrompt = $event"
            />
          </template>
        </PromptField>
        <PromptToolbox
          v-model="promptValue"
          v-model:auto-complete="autoComplete"
          :positive-prompt="store.params.positivePrompt"
          @update:positive-prompt="store.params.positivePrompt = $event"
          @notice="store.status = $event"
        />
        <div class="quality-row">
          <button type="button" class="ghost-text" @click="saveArtistBlock">存画师串</button>
          <button
            type="button"
            class="ghost-text"
            :class="{ on: store.params.transparentBackground }"
            @click="store.params.transparentBackground = !store.params.transparentBackground"
          >
            透明背景
          </button>
          <label class="quality">
            质量词预设
            <select v-model="store.params.qualityPreset">
              <option value="standard">标准</option>
              <option value="light">轻量</option>
              <option value="none">不使用</option>
            </select>
          </label>
        </div>
      </section>

      <section class="block">
        <header class="head char-tools">
          <h2>角色提示词 <em>{{ store.characters.length }}</em></h2>
          <span class="pos-label">定位</span>
          <div class="seg">
            <button type="button" :class="{ on: !store.customPositions }" @click="store.setPositionMode(false)">AI 选择</button>
            <button type="button" :class="{ on: store.customPositions }" @click="store.setPositionMode(true)">自定义</button>
          </div>
          <button
            class="pos-edit"
            type="button"
            :class="{ on: store.positionEditorOpen }"
            title="编辑角色位置"
            @click="store.positionEditorOpen ? store.finishPositionEditor() : store.startPositionEditor()"
          >
            <NaiIcon name="grid" :size="14" />
          </button>
          <div class="add-wrap">
            <button
              class="icon-sq"
              type="button"
              :disabled="!canCharacters"
              title="添加角色"
              @click.stop="charMenu = !charMenu"
            >
              <NaiIcon name="plus" :size="16" />
            </button>
            <div v-if="charMenu" class="char-menu" @click.stop>
              <button type="button" :disabled="store.characters.length >= maxChars" @click="addCharacterKind('girl, ')">
                <NaiIcon name="female" :size="15" /> 添加女性
              </button>
              <button type="button" :disabled="store.characters.length >= maxChars" @click="addCharacterKind('boy, ')">
                <NaiIcon name="male" :size="15" /> 添加男性
              </button>
              <button type="button" :disabled="store.characters.length >= maxChars" @click="addCharacterKind('')">
                <NaiIcon name="user" :size="15" /> 添加其他
              </button>
              <hr />
              <button class="danger" type="button" :disabled="!store.characters.length" @click="clearAllCharacters">
                <NaiIcon name="trash" :size="15" /> 清空全部
              </button>
            </div>
          </div>
        </header>

        <article v-for="(c, i) in store.characters" :key="c.id" class="char" :class="{ off: !c.enabled }">
          <div class="char-head">
            <NaiIcon name="pin" :size="14" />
            <strong>角色 {{ i + 1 }}</strong>
            <span class="grow" />
            <button class="ghost" type="button" @click="store.moveCharacter(c.id, -1)"><NaiIcon name="up" :size="14" /></button>
            <button class="ghost" type="button" @click="store.moveCharacter(c.id, 1)"><NaiIcon name="down" :size="14" /></button>
            <button class="ghost" :class="{ on: c.enabled }" type="button" @click="store.updateCharacter(c.id, { enabled: !c.enabled })">
              <NaiIcon name="check" :size="14" />
            </button>
            <button class="ghost" type="button" @click="store.removeCharacter(c.id)"><NaiIcon name="trash" :size="14" /></button>
            <NaiIcon name="drag" :size="14" />
          </div>
          <div class="text-tabs">
            <button type="button" :class="{ on: (charTab[c.id] || 'prompt') === 'prompt' }" @click="charTab[c.id] = 'prompt'">正面</button>
            <button type="button" :class="{ on: charTab[c.id] === 'uc' }" @click="charTab[c.id] = 'uc'">负面</button>
          </div>
          <PromptField
            v-if="(charTab[c.id] || 'prompt') === 'prompt'"
            :model-value="c.prompt"
            :enabled="autoComplete"
            :targets="tagTargets"
            :target-id="`char:${c.id}:prompt`"
            placeholder="Character prompt..."
            @update:model-value="store.updateCharacter(c.id, { prompt: $event })"
            @place="placeTag"
          >
            <template #tools>
              <DanbooruBubble
                :text="c.prompt"
                @apply="store.updateCharacter(c.id, { prompt: $event })"
              />
            </template>
          </PromptField>
          <PromptField
            v-else
            :model-value="c.negativePrompt"
            :enabled="autoComplete"
            :targets="tagTargets"
            :target-id="`char:${c.id}:uc`"
            placeholder="lowres, extra fingers..."
            @update:model-value="store.updateCharacter(c.id, { negativePrompt: $event })"
            @place="placeTag"
          />
        </article>
      </section>

      <ReferencePanel :on-pick="onPickRef" />

      <section class="block image-set">
        <h2>图像</h2>
        <div class="size-row">
          <select class="family" v-model="family">
            <option value="small">小图</option>
            <option value="normal">常规</option>
            <option value="large">大图</option>
            <option value="wallpaper">壁纸</option>
          </select>
          <div class="ori">
            <button type="button" :class="{ on: orientation === 'landscape' }" title="横图" @click="orientation = 'landscape'">
              <NaiIcon name="landscape" :size="14" />
            </button>
            <button type="button" :class="{ on: orientation === 'square' }" title="方图" @click="orientation = 'square'">
              <NaiIcon name="square" :size="14" />
            </button>
            <button type="button" :class="{ on: orientation === 'portrait' }" title="竖图" @click="orientation = 'portrait'">
              <NaiIcon name="portrait" :size="14" />
            </button>
          </div>
          <input class="px" type="number" :value="store.params.width" @change="store.params.width = Number(($event.target as HTMLInputElement).value) || store.params.width" />
          <span class="times">×</span>
          <input class="px" type="number" :value="store.params.height" @change="store.params.height = Number(($event.target as HTMLInputElement).value) || store.params.height" />
        </div>
        <div class="count-line">
          <span>张数</span>
          <div class="count">
            <button v-for="n in 4" :key="n" type="button" :class="{ on: store.batchCount === n }" @click="store.batchCount = n">{{ n }}</button>
          </div>
        </div>
      </section>
      </div>

      <div class="dock">
        <section v-if="aiOpen" class="drawer">
          <header>
            <h2>生成参数</h2>
            <button type="button" title="收起" @click="aiOpen = false"><NaiIcon name="down" :size="15" /></button>
          </header>
          <div class="drawer-body">
            <div class="pair">
              <label>
                种子
                <span class="seed-box">
                  <input :value="seedDisplay()" placeholder="随机" @input="onSeedInput" />
                  <button type="button" title="改为随机" @click="store.clearSeed()"><NaiIcon name="dice" :size="14" /></button>
                  <button type="button" title="固定种子" :class="{ on: store.params.seedMode === 'fixed' }" @click="toggleSeedPin">
                    <NaiIcon :name="store.params.seedMode === 'fixed' ? 'lock' : 'unlock'" :size="14" />
                  </button>
                </span>
              </label>
              <label>
                采样器
                <select v-model="store.params.sampler">
                  <option v-for="s in NAI_SAMPLERS" :key="s.value" :value="s.value">{{ s.short }}</option>
                </select>
              </label>
            </div>
            <label>
              噪声调度
              <select v-model="store.params.noiseSchedule">
                <option v-for="item in NOISE_SCHEDULES" :key="item.value" :value="item.value">{{ item.label }}</option>
              </select>
            </label>
            <label class="dslide">
              <span>步数</span>
              <input v-model.number="store.params.steps" type="range" min="1" max="50" />
              <b>{{ store.params.steps }}</b>
            </label>
            <label class="dslide">
              <span>引导</span>
              <input v-model.number="store.params.cfgScale" type="range" min="0" max="10" step="0.1" />
              <b>{{ cfgText }}</b>
            </label>
            <label class="dslide">
              <span>引导重缩放</span>
              <input v-model.number="store.params.cfgRescale" type="range" min="0" max="1" step="0.01" />
              <b>{{ rescaleText }}</b>
            </label>
            <button class="chip" type="button" :class="{ on: store.params.variety }" @click="store.params.variety = !store.params.variety">
              <NaiIcon v-if="store.params.variety" name="check" :size="12" />
              Variety+
            </button>
          </div>
        </section>
        <div class="strip">
          <button class="cell" type="button" @click="aiOpen = !aiOpen">
            <small>步数</small>
            <b>{{ store.params.steps }}</b>
          </button>
          <button class="cell" type="button" @click="aiOpen = !aiOpen">
            <small>引导</small>
            <b>{{ cfgText }}</b>
          </button>
          <button class="cell" type="button" @click="toggleSeedPin">
            <small>种子</small>
            <b>{{ seedDisplay() || "随机" }}</b>
          </button>
          <button class="cell grow" type="button" @click="aiOpen = true">
            <small>采样器</small>
            <b>{{ samplerLabel }}</b>
          </button>
          <button class="chev-btn" :class="{ open: aiOpen }" type="button" title="生成参数" @click="aiOpen = !aiOpen">
            <NaiIcon name="up" :size="15" />
          </button>
        </div>
        <OpusUsageBar />
        <button class="generate" type="button" :disabled="store.busy" @click="runGenerate">
          <span>{{ store.busy ? "正在生成…" : generateLabel }}</span>
          <span class="cost"><NaiIcon name="anlas" :size="13" />{{ anlas }}</span>
        </button>
      </div>
    </section>

    <NaiAgentPanel v-if="agentOpen" class="agent-dock" :dock-right="agentRight" @close="closeAgent" @dock="setAgentDock" />

    <section class="center">
      <div class="canvas-bar">
        <button
          type="button"
          class="stream-switch"
          :class="{ on: store.settings.streamPreviewEnabled }"
          role="switch"
          :aria-checked="store.settings.streamPreviewEnabled"
          @click="store.toggleStreamPreview()"
        >
          <NaiIcon name="play" :size="12" />
          流式预览
          <span class="knob" aria-hidden="true"><i /></span>
        </button>
        <button
          ref="guideBtn"
          class="guide-btn"
          type="button"
          :class="{ on: guideOpen || store.overlayGuide !== 'none' }"
          title="构图网格"
          @click.stop="toggleGuide"
        >
          <NaiIcon name="grid" :size="16" />
        </button>
        <Teleport to="body">
          <div v-if="guideOpen" class="guide-pop" :style="guidePopStyle">
            <div class="guide-modes">
              <button type="button" :class="{ on: store.overlayGuide === 'none' }" @click="store.overlayGuide = 'none'">None</button>
              <button type="button" :class="{ on: store.overlayGuide === 'thirds' }" @click="store.overlayGuide = 'thirds'">Thirds</button>
              <button type="button" :class="{ on: store.overlayGuide === 'phi' }" @click="store.overlayGuide = 'phi'">Phi</button>
              <button type="button" :class="{ on: store.overlayGuide === 'grid' }" @click="store.overlayGuide = 'grid'">Grid</button>
            </div>
            <div v-if="store.overlayGuide === 'grid'" class="guide-size">
              <button type="button" @click="bumpGuide('cols', -1)">−</button>
              <b>{{ store.overlayCols }}</b>
              <button type="button" @click="bumpGuide('cols', 1)">+</button>
              <span>×</span>
              <button type="button" @click="bumpGuide('rows', -1)">−</button>
              <b>{{ store.overlayRows }}</b>
              <button type="button" @click="bumpGuide('rows', 1)">+</button>
            </div>
          </div>
        </Teleport>
      </div>
      <div v-if="store.busy || store.streamReplaying" class="stream-stage" :class="{ live: store.genPreview && !store.settings.stealthMode }">
        <img v-if="store.genPreview && !store.settings.stealthMode" :src="store.genPreview" alt="" />
        <div v-else class="stream-wait">
          <i class="spin" />
          <strong>{{ store.genPhase === "waiting" ? "正在连接 NovelAI…" : store.genPhase === "saving" ? "正在保存…" : "Generating…" }}</strong>
        </div>
        <div class="stream-status">
          <strong>{{ streamTitle }}</strong>
          <span>{{ Math.round(displayProgress * 100) }}% · {{ store.genStep }}/{{ store.genSteps }} steps</span>
          <div class="bar"><i :style="{ width: `${Math.max(3, Math.round(displayProgress * 100))}%` }" /></div>
        </div>
      </div>
      <div v-else-if="previewCollapsed && !store.positionEditorOpen && (store.previewUrl || store.i2iImage)" class="preview-stage collapsed">
        <button class="preview-open" type="button" @click="previewCollapsed = false">显示图片</button>
      </div>
      <div v-else-if="store.previewUrl || store.i2iImage || store.positionEditorOpen" class="preview-stage">
        <PreviewToolbar v-if="!store.positionEditorOpen && (store.previewUrl || store.i2iImage)" />
        <div class="preview-frame">
          <img v-if="store.previewUrl || store.i2iImage" :src="store.previewUrl || store.i2iImage" alt="" />
          <div
            v-else
            class="blank-stage"
            :style="{ aspectRatio: `${store.params.width} / ${store.params.height}` }"
          />
          <CompositionGuide />
          <CharacterPositionOverlay v-if="store.positionEditorOpen" />
          <img
            v-if="store.pinnedUrl && store.pinnedUrl !== (store.previewUrl || store.i2iImage) && !store.positionEditorOpen"
            class="pinned"
            :src="store.pinnedUrl"
            alt="已固定"
            title="点击查看固定图"
            @click="store.previewUrl = store.pinnedUrl"
          />
          <button
            v-if="!store.positionEditorOpen && (store.previewUrl || store.i2iImage)"
            class="preview-x"
            type="button"
            title="隐藏图片"
            @click.stop="previewCollapsed = true"
          >
            ×
          </button>
        </div>
        <button
          v-if="store.positionEditorOpen"
          class="finish-pos"
          type="button"
          @click="store.finishPositionEditor()"
        >
          Finish Editing Positions
        </button>
      </div>
      <StarterGallery v-else />
    </section>
    <InpaintEditor v-if="store.paintEditorOpen" />

    <HistoryRail />
    <ImportImageDialog
      v-if="store.importReport"
      :preview="store.importPreview"
      :report="store.importReport"
      @close="store.closeImportDialog()"
    />
  </div>
</template>

<style scoped>
.page { display: flex; height: 100%; min-height: 0; background: var(--bg0); position: relative; overflow: hidden; }
.drop-mask {
  position: fixed; inset: 0; z-index: 45;
  background: rgba(14, 15, 33, 0.58);
  display: grid; place-items: center;
  pointer-events: none;
}
.drop-card {
  width: 92px;
  height: 92px;
  border-radius: 16px;
  background: #1b1e38;
  box-shadow: 0 16px 40px rgba(0, 0, 0, 0.45);
  display: grid;
  place-items: center;
  color: #fff;
}
.left {
  width: 400px;
  min-width: 380px;
  overflow: hidden;
  background: var(--bg1);
  display: flex;
  flex-direction: column;
}
.left-scroll {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 10px 12px 12px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.dock {
  flex: none;
  max-height: 70%;
  overflow: auto;
  padding: 0 12px 12px;
  background: var(--bg1);
  border-top: 1px solid var(--bg3);
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.center {
  position: relative;
  flex: 1;
  min-width: 0;
  overflow: auto;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: flex-start;
  background: var(--bg0);
}
.center img { max-width: 100%; max-height: 100%; object-fit: contain; margin: auto; }
.preview-stage {
  margin: auto;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  max-width: 100%;
  padding: 48px 12px 24px;
}
.preview-frame {
  position: relative;
  display: inline-grid;
  max-width: 100%;
  max-height: 100%;
}
.blank-stage {
  width: min(72vw, 520px);
  max-height: calc(100vh - 160px);
  background: #d8d8e2;
}
.preview-frame img {
  max-width: min(100%, 72vw);
  max-height: calc(100vh - 120px);
  margin: 0;
}
.preview-stage.collapsed { padding-top: 88px; }
.preview-open {
  border: 1px solid var(--bg3);
  background: var(--bg2);
  color: #fff;
  border-radius: 8px;
  padding: 10px 16px;
  font-size: 14px;
}
.preview-open:hover { border-color: var(--heading); color: var(--heading); }
.preview-x {
  position: absolute;
  top: 8px;
  right: 8px;
  z-index: 6;
  width: 28px;
  height: 28px;
  border: 0;
  border-radius: 50%;
  background: rgba(14, 15, 33, 0.78);
  color: #fff;
  font-size: 18px;
  line-height: 1;
}
.preview-x:hover { background: #ff7878; color: #1a1020; }
.preview-frame .pinned {
  position: absolute;
  left: 10px;
  bottom: 10px;
  width: 92px;
  height: auto;
  max-height: 128px;
  border-radius: 8px;
  border: 2px solid #f3e27a;
  box-shadow: 0 8px 20px rgba(0,0,0,0.45);
  cursor: pointer;
}
.paint-bar {
  width: 100%;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  padding: 10px 14px 6px;
  color: #fff;
  font-size: 13px;
}
.paint-bar button {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  background: var(--bg2);
  border: 1px solid var(--bg3);
  color: #fff;
  border-radius: 8px;
  padding: 6px 10px;
}
.paint-bar button.on { color: var(--heading); border-color: var(--heading); }
.paint-bar button:disabled { opacity: 0.35; }
.paint-bar .brush { display: inline-flex; align-items: center; gap: 8px; color: rgba(255,255,255,0.8); }
.paint-bar .brush input { width: 120px; }
.paint-bar .hint { color: var(--muted); }
.canvas-bar {
  position: absolute;
  top: 14px;
  right: 16px;
  z-index: 8;
  display: flex;
  align-items: center;
  gap: 8px;
}
.stream-switch {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  height: 32px;
  padding: 0 8px 0 10px;
  border: 1px solid #2b2e4a;
  border-radius: 999px;
  background: #1a1c34;
  color: rgba(255,255,255,0.78);
  font-size: 12px;
  font-weight: 650;
}
.stream-switch.on {
  color: var(--heading);
  border-color: color-mix(in srgb, var(--heading) 35%, #2b2e4a);
  background: color-mix(in srgb, var(--heading) 10%, #1a1c34);
}
.stream-switch .knob {
  position: relative;
  width: 28px;
  height: 16px;
  border-radius: 99px;
  background: #3a3d5c;
}
.stream-switch .knob i {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: #fff;
  transition: transform 0.16s ease;
}
.stream-switch.on .knob { background: var(--heading); }
.stream-switch.on .knob i { transform: translateX(12px); background: #1a1c34; }
.stream-stage {
  width: 100%;
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-rows: minmax(0, 1fr) auto;
  align-content: stretch;
  gap: 12px;
  padding: 52px 24px 24px;
}
.stream-stage img {
  width: 100%;
  height: 100%;
  object-fit: contain;
  border-radius: 8px;
  box-shadow: 0 18px 40px rgba(0, 0, 0, 0.35);
}
.stream-wait {
  display: grid;
  place-items: center;
  align-content: center;
  gap: 14px;
  color: var(--muted);
  font-family: Eczar, serif;
  font-size: 26px;
}
.stream-wait .spin {
  width: 28px;
  height: 28px;
  border: 2px solid #2b2e4a;
  border-top-color: var(--heading);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}
@keyframes spin { to { transform: rotate(360deg); } }
.stream-status {
  width: min(520px, 100%);
  justify-self: center;
  display: grid;
  grid-template-columns: 1fr auto;
  gap: 4px 12px;
  padding: 10px 12px;
  border: 1px solid #2b2e4a;
  border-radius: 10px;
  background: #1a1c34;
  color: #fff;
}
.stream-status strong { font-size: 13px; }
.stream-status span { color: var(--muted); font-size: 12px; }
.stream-status .bar {
  grid-column: 1 / -1;
  height: 4px;
  border-radius: 99px;
  background: #22253f;
  overflow: hidden;
}
.stream-status .bar i {
  display: block;
  height: 100%;
  background: var(--heading);
  transition: width 0.16s ease-out;
}
.model, select, input, textarea {
  background: var(--bg0);
  border: 1px solid var(--bg3);
  border-radius: 4px;
  padding: 8px 10px;
  color: #fff;
}
.model-row { display: flex; }
.model-capsule {
  display: inline-flex;
  align-items: center;
  height: 32px;
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.06);
}
.model-capsule > span {
  padding-left: 10px;
  color: rgba(255, 255, 255, 0.55);
  font-size: 12px;
}
.model-capsule .split {
  width: 1px;
  height: 16px;
  background: rgba(255, 255, 255, 0.18);
}
.model-capsule .mode {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  height: 32px;
  padding: 0 10px 0 8px;
  border: 0;
  outline: none;
  box-shadow: none;
  background: transparent;
  color: rgba(255, 255, 255, 0.86);
  font-weight: 650;
  cursor: pointer;
}
.model-capsule .mode:hover,
.model-capsule .model:hover {
  background: rgba(255, 255, 255, 0.1);
  color: #fff;
}
.model-capsule .mode:focus,
.model-capsule .mode:focus-visible,
.model-capsule .model:focus,
.model-capsule .model:focus-visible {
  outline: none;
  border: 0;
  box-shadow: none;
}
.model-pick { position: relative; display: flex; align-self: stretch; }
.model-capsule .model {
  width: auto;
  max-width: none;
  height: 32px;
  padding: 0 8px 0 10px;
  border: 0;
  border-radius: 0;
  outline: none;
  box-shadow: none;
  background: transparent;
  color: #fff;
  font-weight: 700;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  cursor: pointer;
}
.model-menu {
  position: absolute;
  z-index: 40;
  top: 100%;
  left: 0;
  min-width: 148px;
  margin: 0;
  padding: 4px;
  background: var(--bg1);
  border: 0;
  outline: none;
  border-radius: 0 0 8px 8px;
  box-shadow: 0 14px 28px rgba(0, 0, 0, 0.28);
  overflow: hidden;
}
.model-menu button {
  display: flex;
  align-items: center;
  width: 100%;
  height: 30px;
  padding: 0 10px;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: rgba(255, 255, 255, 0.86);
  text-align: left;
  cursor: pointer;
}
.model-menu button:hover { background: rgba(255, 255, 255, 0.08); color: #fff; }
.model-menu button.on { background: rgba(255, 255, 255, 0.12); color: #fff; }
.chips { display: flex; gap: 8px; flex-wrap: wrap; }
.chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  background: var(--bg2);
  border: 1px solid var(--bg3);
  border-radius: 4px;
  padding: 6px 10px;
  font-size: 12px;
  color: #fff;
}
.chip.on { background: var(--chip); color: var(--bg0); }
.chip.select { padding-right: 6px; }
.chip select { background: transparent; border: 0; padding: 0 4px; color: inherit; }
.block {
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 12px;
  padding: 10px 12px 12px;
  background: rgba(255, 255, 255, 0.02);
}
.head { display: flex; justify-content: space-between; align-items: center; gap: 6px; }
.char-tools { flex-wrap: nowrap; }
.char-tools h2 { display: flex; align-items: center; gap: 6px; margin-right: 4px; font-size: 14px; font-family: inherit; letter-spacing: 0; font-weight: 650; white-space: nowrap; }
.char-tools h2 em {
  min-width: 18px;
  height: 18px;
  padding: 0 5px;
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.08);
  color: var(--muted);
  font-style: normal;
  font-size: 11px;
  line-height: 18px;
  text-align: center;
}
.pos-label { color: var(--muted); font-size: 12px; }
.quality-row { display: flex; align-items: center; justify-content: flex-end; gap: 10px; margin-top: 4px; }
.ghost-text {
  margin-right: auto;
  border: 0;
  background: transparent;
  color: var(--muted);
  font-size: 12px;
  font-weight: 500;
  padding: 0;
}
.ghost-text.on { color: var(--heading); }
.quality { display: flex; align-items: center; gap: 4px; color: var(--muted); font-size: 12px; }
.quality select { height: auto; border: 0; background: transparent; padding: 0; color: #fff; font-weight: 600; }
.quality select option,
.model option,
.family option,
select option { color: #1a1c2e; background: #fff; }
.add-wrap { position: relative; }
.char-menu {
  position: absolute;
  top: calc(100% + 6px);
  right: 0;
  z-index: 30;
  min-width: 168px;
  padding: 6px;
  background: #1c2038;
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 10px;
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.45);
  display: grid;
  gap: 2px;
}
.char-menu button {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 34px;
  padding: 0 8px;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: #f4f6ff;
  font-size: 13px;
  text-align: left;
}
.char-menu button:hover:not(:disabled) { background: rgba(255, 255, 255, 0.08); }
.char-menu button:disabled { opacity: 0.4; }
.char-menu hr { border: 0; border-top: 1px solid rgba(255, 255, 255, 0.1); margin: 4px 2px; }
.char-menu .danger { color: #ff6b6b; }
.block h2, .image-set h2 { font-size: 14px; margin: 0 0 8px; font-family: inherit; letter-spacing: 0; font-weight: 650; }
.block p { margin: 2px 0 10px; color: var(--muted); font-size: 12px; font-weight: 400; }
.icon-sq {
  width: 34px; height: 34px; display: grid; place-items: center;
  background: var(--bg2); border: 1px solid var(--bg3); border-radius: 6px; color: #fff;
}
.icon-sq.tiny { width: 28px; height: 28px; }
.icon-sq.on, .ghost.on { color: var(--heading); border-color: var(--heading); }
.icon-sq:disabled { opacity: 0.4; }
.pos { display: flex; align-items: center; gap: 8px; margin: 8px 0; font-size: 13px; color: var(--muted); }
.seg {
  display: flex;
  margin-left: 0;
  background: transparent;
  border: 0;
  border-radius: 8px;
  padding: 0;
  gap: 2px;
}
.seg button {
  background: transparent;
  border: 0;
  border-radius: 8px;
  color: var(--muted);
  padding: 4px 8px;
  font-size: 12px;
  font-weight: 600;
}
.seg button.on {
  background: rgba(255, 255, 255, 0.12);
  color: #fff;
}
.pos-edit {
  width: 32px;
  height: 32px;
  display: grid;
  place-items: center;
  background: transparent;
  border: 0;
  border-radius: 8px;
  color: rgba(255,255,255,0.72);
}
.pos-edit.on, .pos-edit:hover { color: #fff; background: var(--bg2); }
.char {
  margin-top: 10px;
  padding: 8px 8px 4px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 10px;
}
.char.off { opacity: 0.5; }
.char-head { display: flex; align-items: center; gap: 6px; color: #fff; font-size: 13px; }
.grow { flex: 1; }
.ghost {
  background: transparent; border: 0; color: rgba(255,255,255,0.7); padding: 2px; display: grid; place-items: center;
}
.text-tabs { display: flex; align-items: center; gap: 12px; margin: 8px 0 6px; }
.text-tabs button {
  background: none; border: 0; padding: 4px 8px; color: var(--muted); font-size: 13px; border-radius: 8px; font-weight: 600;
}
.text-tabs button.on { color: #fff; background: rgba(255, 255, 255, 0.1); }
.text-tabs .agent-slot {
  margin-left: auto;
  width: 28px;
  height: 28px;
  padding: 0;
  background: transparent;
}
.text-tabs .agent-slot:hover { background: rgba(47, 123, 255, 0.16); }
.agent-dock { width: 400px; min-width: 340px; flex: none; min-height: 0; }
.page.agent-right .center { order: 1; }
.page.agent-right .agent-dock { order: 2; }
.page.agent-right :deep(.rail) { right: 400px; }
.block textarea {
  width: 100%; min-height: 88px; resize: vertical; background: var(--bg0); border: 0; border-radius: 6px; padding: 10px;
}
.image-set h2 { margin-bottom: 10px; }
.size-row { display: flex; align-items: center; gap: 6px; }
.family {
  width: 84px;
  height: 32px;
  padding: 0 8px;
  border: 0;
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.06);
}
.ori { display: flex; gap: 4px; }
.ori button {
  width: 32px;
  height: 32px;
  display: grid;
  place-items: center;
  border: 0;
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.06);
  color: rgba(255, 255, 255, 0.62);
}
.ori button.on { background: rgba(255, 255, 255, 0.16); color: #fff; }
.px {
  width: 58px;
  height: 32px;
  padding: 0 4px;
  border: 0;
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.06);
  text-align: center;
  font-variant-numeric: tabular-nums;
}
.times { color: var(--muted); font-size: 12px; }
.count-line { display: flex; align-items: center; gap: 10px; margin-top: 10px; }
.count-line > span { color: var(--muted); font-size: 12px; flex: none; }
.ai { display: flex; flex-direction: column; }
.count {
  flex: 1;
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  height: 32px;
  background: rgba(255, 255, 255, 0.06);
  border: 0;
  border-radius: 8px;
  overflow: hidden;
}
.count button {
  background: transparent;
  border: 0;
  border-right: 1px solid var(--bg3);
  color: rgba(255,255,255,0.55);
  font-weight: 700;
}
.count button:last-child { border-right: 0; }
.count button.on {
  background: #22253f;
  color: #fff;
}
.slide { display: grid; grid-template-columns: 1fr auto; gap: 4px 8px; margin: 10px 0; font-size: 13px; color: rgba(255,255,255,0.8); }
.slide input[type="range"] { grid-column: 1 / -1; width: 100%; }
.slide b {
  background: var(--bg0);
  border-radius: 6px;
  padding: 2px 8px;
  min-width: 28px;
  text-align: center;
  font-variant-numeric: tabular-nums;
}
.seed-row { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; }
.seed-row span { display: block; color: var(--muted); font-size: 12px; margin-bottom: 6px; }
.seed-box { display: flex; gap: 6px; align-items: center; }
.seed-box input { flex: 1; }
.adv { background: none; border: 0; color: var(--muted); padding: 6px 0; text-align: left; }
.extra label { display: grid; gap: 4px; font-size: 12px; color: var(--muted); }
.drawer {
  margin: 0 -12px;
  background: #16182e;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-bottom: 0;
  border-radius: 10px 10px 0 0;
  box-shadow: 0 -10px 24px rgba(0, 0, 0, 0.35);
}
.drawer header {
  height: 40px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 6px 0 14px;
}
.drawer h2 { margin: 0; font-size: 13px; font-weight: 700; font-family: inherit; letter-spacing: 0; }
.drawer header button, .seed-box button, .chev-btn {
  width: 28px;
  height: 28px;
  display: grid;
  place-items: center;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: rgba(255, 255, 255, 0.72);
}
.drawer header button:hover, .seed-box button:hover, .chev-btn:hover { background: rgba(255, 255, 255, 0.08); }
.seed-box button.on { color: #8eb7ff; }
.drawer-body { display: flex; flex-direction: column; gap: 12px; padding: 2px 14px 14px; }
.drawer-body > label, .pair label {
  display: flex;
  flex-direction: column;
  gap: 6px;
  color: var(--muted);
  font-size: 12px;
}
.pair { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; }
.seed-box, .drawer-body select {
  height: 32px;
  display: flex;
  align-items: center;
  gap: 2px;
  padding: 0 2px 0 10px;
  border: 0;
  border-radius: 6px;
  background: rgba(255, 255, 255, 0.06);
  color: #fff;
}
.drawer-body select { width: 100%; padding-right: 8px; }
.seed-box input {
  flex: 1;
  min-width: 0;
  height: 100%;
  padding: 0;
  border: 0;
  background: transparent;
  color: #fff;
}
.dslide {
  display: grid;
  grid-template-columns: 72px 1fr auto;
  align-items: center;
  gap: 10px;
  color: #fff;
  font-size: 13px;
}
.dslide input { width: 100%; }
.dslide b {
  min-width: 42px;
  padding: 2px 6px;
  border-radius: 6px;
  background: rgba(255, 255, 255, 0.06);
  text-align: center;
  font-variant-numeric: tabular-nums;
  font-size: 12px;
}
.chip {
  align-self: flex-start;
  height: 30px;
  padding: 0 12px;
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 999px;
  background: transparent;
  color: rgba(255, 255, 255, 0.82);
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
}
.chip.on { background: rgba(255, 255, 255, 0.1); color: #fff; }
.strip {
  display: flex;
  align-items: center;
  min-height: 46px;
  margin: 0 -4px;
}
.strip .cell {
  min-width: 52px;
  padding: 6px 10px;
  border: 0;
  border-radius: 5px;
  background: transparent;
  color: #fff;
  text-align: left;
}
.strip .cell.grow { flex: 1; min-width: 0; }
.strip .cell:hover { background: rgba(255, 255, 255, 0.06); }
.strip small {
  display: block;
  color: rgba(255, 255, 255, 0.55);
  font-size: 11px;
  font-weight: 500;
  line-height: 1.4;
}
.strip b {
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12px;
  font-weight: 500;
  line-height: 1.4;
}
.chev-btn { flex: none; margin-right: 6px; border: 1px solid rgba(255, 255, 255, 0.14); background: rgba(255, 255, 255, 0.04); }
.chev-btn.open { transform: rotate(180deg); }
.generate {
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: space-between;
  background: #f5f3c2;
  color: var(--bg0);
  border: 0;
  border-radius: 8px;
  padding: 10px 10px 10px 14px;
  font-weight: 700;
}
.guide-btn {
  width: 32px;
  height: 32px;
  display: grid;
  place-items: center;
  background: #1a1c34;
  border: 1px solid #2b2e4a;
  border-radius: 8px;
  color: rgba(255,255,255,0.78);
}
.guide-btn.on { color: var(--heading); border-color: var(--heading); }
.guide-pop {
  position: fixed;
  min-width: 260px;
  padding: 10px;
  border-radius: 12px;
  background: #151728;
  border: 1px solid #2a2d48;
  box-shadow: 0 12px 28px rgba(0,0,0,0.4);
  z-index: 40;
}
.guide-modes {
  display: flex;
  background: var(--bg0);
  border-radius: 999px;
  padding: 3px;
}
.guide-modes button {
  flex: 1;
  border: 0;
  background: transparent;
  color: #fff;
  border-radius: 999px;
  padding: 6px 0;
  font-size: 12px;
}
.guide-modes button.on { background: #fff; color: #1a1c2e; font-weight: 700; }
.guide-size {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  margin-top: 10px;
  color: #fff;
}
.guide-size button {
  width: 26px;
  height: 26px;
  border: 0;
  border-radius: 6px;
  background: var(--bg2);
  color: #fff;
}
.guide-size b { min-width: 16px; text-align: center; }
.finish-pos {
  background: #f5f3c2;
  color: var(--bg0);
  border: 0;
  border-radius: 999px;
  padding: 10px 16px;
  font-weight: 700;
}
.generate:disabled { opacity: 0.55; }
.cost {
  display: inline-flex; align-items: center; gap: 4px;
  background: #0e0f21; color: #fff; border-radius: 8px; padding: 6px 10px; font-size: 13px;
}
.ai-acts { display: flex; gap: 4px; }
</style>
