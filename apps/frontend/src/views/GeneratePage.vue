<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useRoute } from "vue-router";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { NAI_MODELS, NAI_SAMPLERS, NAI_UC_PRESETS, isV4Plus, maxCharacterPrompts, supportsNAIPreciseReference, supportsNAIVibeTransfer } from "@/types/nai";
import { useAppStore } from "@/stores/app";
import { fetchRemoteImage } from "@/api/tauri";
import { quoteAnlas } from "@/utils/anlas";
import { RESOLUTION_LABELS, applyResolution, inferResolution, type Orientation, type ResolutionFamily } from "@/utils/resolution";
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
import { loadAutoComplete } from "@/utils/promptTools";

const store = useAppStore();
const route = useRoute();
const aiOpen = ref(false);
const advanced = ref(false);
const dropping = ref(false);
const guideOpen = ref(false);
const promptTab = ref<"prompt" | "uc">("prompt");
const charTab = ref<Record<string, "prompt" | "uc">>({});
const autoComplete = ref(loadAutoComplete());
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
const actionKind = computed(() =>
  inpaintOn.value ? "infill" : store.i2iImage ? "img2img" : "generate",
);
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
  if (inpaintOn.value) return n === 1 ? "Inpaint 1 Image" : `Inpaint ${n} Images`;
  return n === 1 ? "Generate 1 Image" : `Generate ${n} Images`;
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

function applyRefFile(file: File, kind: "i2i" | "vibe" | "precise") {
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

function onPickRef(file: File, kind?: "i2i" | "vibe" | "precise") {
  if (kind) {
    applyRefFile(file, kind);
    return;
  }
  void store.openImportFromFile(file);
}

function dropKindFromTarget(target: EventTarget | null): "i2i" | "vibe" | "precise" | undefined {
  const el = target instanceof Element ? target.closest("[data-ref-drop]") : null;
  const kind = el?.getAttribute("data-ref-drop");
  if (kind === "i2i" || kind === "vibe" || kind === "precise") return kind;
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

async function importDropped(kind: "i2i" | "vibe" | "precise" | undefined, file?: File, url?: string, localPath?: string) {
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
  ev.preventDefault();
  void store.openImportFromFile(file);
}

let unlistenTauri: (() => void) | undefined;
function onPosKey(ev: KeyboardEvent) {
  if (ev.key === "Escape" && store.positionEditorOpen) {
    ev.preventDefault();
    store.finishPositionEditor();
  }
}

onMounted(() => {
  window.addEventListener("keydown", onPosKey);
  window.addEventListener("paste", onPaste);
  window.addEventListener("dragenter", onWinDragEnter, true);
  window.addEventListener("dragover", onWinDragOver, true);
  window.addEventListener("dragleave", onWinDragLeave, true);
  window.addEventListener("drop", onWinDrop, true);
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
});
onUnmounted(() => {
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

function resetAi() {
  store.params.steps = 28;
  store.params.cfgScale = 5;
  store.params.cfgRescale = 0;
  store.params.sampler = "k_euler_ancestral";
  store.clearSeed();
}
</script>

<template>
  <div class="page">
    <div v-if="dropping" class="drop-mask">
      <div class="drop-card">
        <NaiIcon name="upload" :size="40" />
      </div>
    </div>
    <section class="left">
      <div class="left-scroll">
      <select class="model" v-model="store.params.model">
        <option v-for="m in NAI_MODELS" :key="m.value" :value="m.value">{{ m.label }}</option>
      </select>

      <div class="chips">
        <button
          type="button"
          class="chip"
          :class="{ on: store.params.transparentBackground }"
          @click="store.params.transparentBackground = !store.params.transparentBackground"
        >
          <NaiIcon v-if="store.params.transparentBackground" name="x" :size="12" />
          Transparent BG
        </button>
        <label class="chip select">
          Quality Tags:
          <select v-model="store.params.qualityPreset">
            <option value="standard">Standard</option>
            <option value="light">Light</option>
            <option value="none">None</option>
          </select>
        </label>
      </div>

      <section class="block">
        <div class="text-tabs">
          <button type="button" :class="{ on: promptTab === 'prompt' }" @click="promptTab = 'prompt'">Prompt</button>
          <button type="button" :class="{ on: promptTab === 'uc' }" @click="promptTab = 'uc'">Undesired Content</button>
        </div>
        <PromptField
          v-model="promptValue"
          :enabled="autoComplete"
          :placeholder="promptTab === 'prompt' ? 'Enter your prompt here...' : 'lowres, worst quality...'"
        />
        <PromptToolbox
          v-model="promptValue"
          v-model:auto-complete="autoComplete"
          :positive-prompt="store.params.positivePrompt"
          @update:positive-prompt="store.params.positivePrompt = $event"
          @notice="store.status = $event"
        />
      </section>

      <section class="block">
        <header class="head">
          <div>
            <h2>Character Prompts</h2>
            <p>Create a separate prompt for characters in your scene.</p>
          </div>
          <button
            class="icon-sq"
            type="button"
            :disabled="!canCharacters || store.characters.length >= maxChars"
            @click="store.addCharacter()"
          >
            <NaiIcon name="plus" :size="18" />
          </button>
        </header>

        <div class="pos">
          <span>Position</span>
          <div class="seg">
            <button type="button" :class="{ on: !store.customPositions }" @click="store.setPositionMode(false)">AI’s Choice</button>
            <button type="button" :class="{ on: store.customPositions }" @click="store.setPositionMode(true)">Custom</button>
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
        </div>

        <article v-for="(c, i) in store.characters" :key="c.id" class="char" :class="{ off: !c.enabled }">
          <div class="char-head">
            <NaiIcon name="pin" :size="14" />
            <strong>Character {{ i + 1 }}</strong>
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
            <button type="button" :class="{ on: (charTab[c.id] || 'prompt') === 'prompt' }" @click="charTab[c.id] = 'prompt'">Prompt</button>
            <button type="button" :class="{ on: charTab[c.id] === 'uc' }" @click="charTab[c.id] = 'uc'">Undesired Content</button>
          </div>
          <PromptField
            v-if="(charTab[c.id] || 'prompt') === 'prompt'"
            :model-value="c.prompt"
            :enabled="autoComplete"
            placeholder="Character prompt..."
            @update:model-value="store.updateCharacter(c.id, { prompt: $event })"
          />
          <PromptField
            v-else
            :model-value="c.negativePrompt"
            :enabled="autoComplete"
            placeholder="lowres, extra fingers..."
            @update:model-value="store.updateCharacter(c.id, { negativePrompt: $event })"
          />
        </article>
      </section>

      <ReferencePanel :on-pick="onPickRef" />

      <section class="image-set">
        <h2>Image Settings</h2>
        <div class="res-label">
          <span>Resolution</span>
          <em>{{ store.params.width }} × {{ store.params.height }}</em>
        </div>
        <div class="res-row">
          <select class="family" v-model="family">
            <option v-for="r in RESOLUTION_LABELS" :key="r.id" :value="r.id">{{ r.label }}</option>
          </select>
          <div class="orient">
            <button type="button" :class="{ on: orientation === 'landscape' }" @click="orientation = 'landscape'">
              <NaiIcon name="landscape" :size="14" />
            </button>
            <button type="button" :class="{ on: orientation === 'portrait' }" @click="orientation = 'portrait'">
              <NaiIcon name="portrait" :size="14" />
            </button>
            <button type="button" :class="{ on: orientation === 'square' }" @click="orientation = 'square'">
              <NaiIcon name="square" :size="14" />
            </button>
          </div>
        </div>
        <div class="count-label">Number of Images</div>
        <div class="count">
          <button v-for="n in 4" :key="n" type="button" :class="{ on: store.batchCount === n }" @click="store.batchCount = n">{{ n }}</button>
        </div>
      </section>
      </div>

      <div class="dock">
      <section class="ai">
        <header v-if="aiOpen" class="head">
          <h2>AI Settings</h2>
          <div class="ai-acts">
            <button class="ghost" type="button" @click="resetAi"><NaiIcon name="reset" :size="14" /></button>
            <button class="ghost" type="button" @click="aiOpen = false"><NaiIcon name="down" :size="14" /></button>
          </div>
        </header>

        <div v-if="!aiOpen" class="compact" @click="aiOpen = true">
          <div class="cell">
            <small>Steps</small>
            <b>{{ store.params.steps }}</b>
          </div>
          <div class="cell">
            <small>Guidance</small>
            <b>{{ store.params.cfgScale }}</b>
          </div>
          <div class="cell seed-cell">
            <small>Seed</small>
            <b v-if="seedDisplay()">{{ seedDisplay() }}</b>
            <NaiIcon v-else name="dice" :size="13" />
          </div>
          <div class="cell grow">
            <small>Sampler</small>
            <b>{{ samplerLabel }}</b>
          </div>
          <button class="play" type="button" title="展开 AI Settings" @mousedown.stop @click.stop.prevent="aiOpen = true">
            <NaiIcon name="play" :size="13" />
          </button>
        </div>

        <template v-else>
          <label class="slide">
            <span>Steps</span><b>{{ store.params.steps }}</b>
            <input v-model.number="store.params.steps" type="range" min="1" max="50" />
          </label>
          <label class="slide">
            <span>Prompt Guidance</span><b>{{ store.params.cfgScale }}</b>
            <input v-model.number="store.params.cfgScale" type="range" min="0" max="10" step="0.1" />
          </label>
          <div class="seed-row">
            <div>
              <span>Seed</span>
              <div class="seed-box">
                <input :value="seedDisplay()" placeholder="Enter a seed" @input="onSeedInput" />
                <button class="ghost" type="button" @click="store.rollSeed()"><NaiIcon name="dice" :size="16" /></button>
              </div>
            </div>
            <div>
              <span>Sampler</span>
              <select v-model="store.params.sampler">
                <option v-for="s in NAI_SAMPLERS" :key="s.value" :value="s.value">{{ s.short }}</option>
              </select>
            </div>
          </div>
          <button class="adv" type="button" @click="advanced = !advanced">Advanced Settings</button>
          <label v-if="advanced" class="slide">
            <span>Prompt Guidance Rescale</span><b>{{ store.params.cfgRescale }}</b>
            <input v-model.number="store.params.cfgRescale" type="range" min="0" max="1" step="0.01" />
          </label>
          <div v-if="advanced" class="extra">
            <label>UC Preset
              <select v-model.number="store.params.ucPreset">
                <option v-for="u in NAI_UC_PRESETS" :key="u.value" :value="u.value">{{ u.label }}</option>
              </select>
            </label>
          </div>
        </template>

        <OpusUsageBar />
        <div class="gen-row">
          <button class="generate" type="button" :disabled="store.busy" @click="runGenerate">
            <span>{{ store.busy ? "Generating…" : generateLabel }}</span>
            <span class="cost"><NaiIcon name="anlas" :size="13" />{{ anlas }}</span>
          </button>
          <div class="guide-wrap">
            <button
              class="guide-btn"
              type="button"
              :class="{ on: guideOpen || store.overlayGuide !== 'none' }"
              title="构图网格"
              @click="guideOpen = !guideOpen"
            >
              <NaiIcon name="grid" :size="16" />
            </button>
            <div v-if="guideOpen" class="guide-pop">
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
          </div>
        </div>
      </section>
      </div>
    </section>

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
      </div>
      <div v-if="store.busy || store.streamReplaying" class="stream-stage" :class="{ live: store.genPreview }">
        <img v-if="store.genPreview" :src="store.genPreview" alt="" />
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
  padding: 8px 14px 12px;
  display: flex;
  flex-direction: column;
  gap: 14px;
}
.dock {
  flex: none;
  max-height: 58%;
  overflow: auto;
  padding: 10px 14px 14px;
  background: var(--bg1);
  border-top: 1px solid var(--bg3);
  display: flex;
  flex-direction: column;
  gap: 10px;
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
.model { width: 100%; }
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
.block { padding-top: 4px; }
.head { display: flex; justify-content: space-between; align-items: flex-start; gap: 8px; }
.block h2 { font-size: 20px; margin: 0; }
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
  margin-left: auto;
  background: var(--bg2);
  border: 1px solid var(--bg3);
  border-radius: 999px;
  padding: 2px;
}
.seg button {
  background: transparent;
  border: 0;
  border-radius: 999px;
  color: #fff;
  padding: 6px 12px;
}
.seg button.on {
  background: #fff;
  color: #1a1c2e;
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
.char { margin-top: 8px; }
.char.off { opacity: 0.5; }
.char-head { display: flex; align-items: center; gap: 6px; color: #fff; font-size: 13px; }
.grow { flex: 1; }
.ghost {
  background: transparent; border: 0; color: rgba(255,255,255,0.7); padding: 2px; display: grid; place-items: center;
}
.text-tabs { display: flex; gap: 12px; margin: 8px 0 6px; }
.text-tabs button {
  background: none; border: 0; padding: 0; color: var(--muted); font-size: 13px;
}
.text-tabs button.on { color: var(--heading); }
.block textarea {
  width: 100%; min-height: 88px; resize: vertical; background: var(--bg0); border: 0; border-radius: 6px; padding: 10px;
}
.image-set h2 { font-size: 18px; margin: 0 0 8px; }
.ai { display: flex; flex-direction: column; }
.res-label {
  display: flex;
  align-items: center;
  justify-content: space-between;
  color: #fff;
  font-size: 14px;
  margin-bottom: 8px;
}
.res-label em {
  font-style: normal;
  font-size: 12px;
  color: rgba(255,255,255,0.78);
  background: var(--bg0);
  border-radius: 8px;
  padding: 5px 10px;
  font-variant-numeric: tabular-nums;
}
.res-row {
  display: flex;
  align-items: center;
  gap: 8px;
}
.family {
  width: 118px;
  height: 36px;
  padding: 0 10px;
  border-radius: 8px;
  background: var(--bg0);
  border: 1px solid var(--bg3);
}
.orient {
  display: flex;
  flex: 1;
  height: 36px;
  background: var(--bg0);
  border: 1px solid var(--bg3);
  border-radius: 8px;
  overflow: hidden;
}
.orient button {
  flex: 1;
  display: grid;
  place-items: center;
  background: transparent;
  border: 0;
  color: rgba(255,255,255,0.55);
  border-right: 1px solid var(--bg3);
}
.orient button:last-child { border-right: 0; }
.orient button.on {
  background: #e8e4f0;
  color: var(--bg0);
}
.count-label { color: #fff; font-size: 14px; margin: 12px 0 8px; }
.count {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  height: 36px;
  background: var(--bg0);
  border: 1px solid var(--bg3);
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
.compact {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 12px;
  background: var(--bg0);
  border: 1px solid var(--bg3);
  border-radius: 12px;
  cursor: pointer;
}
.compact .cell { min-width: 48px; }
.compact .cell.grow { flex: 1; min-width: 92px; }
.compact .seed-cell { min-width: 36px; }
.compact small {
  display: block;
  color: var(--muted);
  font-size: 11px;
  font-weight: 600;
  margin-bottom: 2px;
}
.compact b {
  display: block;
  font-size: 15px;
  font-weight: 700;
  line-height: 1.15;
  font-variant-numeric: tabular-nums;
}
.compact .seed-cell :deep(.nai-icon) { color: rgba(255,255,255,0.7); }
.play {
  margin-left: auto;
  width: 22px;
  height: 22px;
  border: 0;
  border-radius: 4px;
  background: #191b31;
  color: #fff;
  display: grid;
  place-items: center;
  flex: none;
  cursor: pointer;
}
.gen-row {
  display: flex;
  align-items: stretch;
  gap: 8px;
  margin-top: 8px;
}
.generate {
  flex: 1; display: flex; align-items: center; justify-content: space-between;
  background: #f5f3c2; color: var(--bg0); border: 0; border-radius: 8px; padding: 10px 10px 10px 14px; font-weight: 700;
}
.guide-wrap { position: relative; flex: none; }
.guide-btn {
  width: 42px;
  height: 100%;
  min-height: 42px;
  display: grid;
  place-items: center;
  background: var(--bg2);
  border: 1px solid var(--bg3);
  border-radius: 8px;
  color: rgba(255,255,255,0.78);
}
.guide-btn.on { color: var(--heading); border-color: var(--heading); }
.guide-pop {
  position: absolute;
  right: 0;
  bottom: calc(100% + 8px);
  min-width: 260px;
  padding: 10px;
  border-radius: 12px;
  background: #151728;
  border: 1px solid #2a2d48;
  box-shadow: 0 12px 28px rgba(0,0,0,0.4);
  z-index: 8;
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
