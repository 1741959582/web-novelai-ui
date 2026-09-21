<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { NAI_MODELS, NAI_SAMPLERS, NAI_UC_PRESETS, isV4Plus, maxCharacterPrompts, supportsNAIPreciseReference, supportsNAIVibeTransfer } from "@/types/nai";
import { useAppStore } from "@/stores/app";
import { quoteAnlas } from "@/utils/anlas";
import { RESOLUTION_LABELS, applyResolution, inferResolution, type Orientation, type ResolutionFamily } from "@/utils/resolution";
import HistoryRail from "@/components/HistoryRail.vue";
import NaiIcon from "@/components/NaiIcon.vue";
import StarterGallery from "@/components/StarterGallery.vue";
import ImportImageDialog from "@/components/ImportImageDialog.vue";
import ReferencePanel from "@/components/ReferencePanel.vue";

const store = useAppStore();
const aiOpen = ref(false);
const advanced = ref(false);
const dropping = ref(false);
const promptTab = ref<"prompt" | "uc">("prompt");
const charTab = ref<Record<string, "prompt" | "uc">>({});
const dragging = ref<string | null>(null);

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

const actionKind = computed(() => (store.i2iImage ? "img2img" : "generate") as "generate" | "img2img");
const anlas = computed(() =>
  quoteAnlas({
    params: store.params,
    account: store.account,
    batchCount: store.batchCount,
    action: actionKind.value,
    strength: store.i2iStrength,
    vibeCount: supportsNAIVibeTransfer(store.params.model) ? store.vibeImages.filter((v) => v.enabled).length : 0,
    preciseCount: supportsNAIPreciseReference(store.params.model) ? store.preciseReferences.filter((p) => p.enabled).length : 0,
  }),
);
const generateLabel = computed(() =>
  store.batchCount === 1 ? "Generate 1 Image" : `Generate ${store.batchCount} Images`,
);
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

function onPickRef(file: File, kind?: "i2i" | "vibe" | "precise") {
  if (kind === "vibe") {
    void fileToDataUrl(file).then((url) => store.addVibeFromDataUrl(url));
    return;
  }
  if (kind === "precise") {
    void fileToDataUrl(file).then((url) => store.addPreciseFromDataUrl(url));
    return;
  }
  void store.openImportFromFile(file);
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
function hasFiles(ev: DragEvent) {
  return [...(ev.dataTransfer?.types ?? [])].includes("Files");
}
function onWinDragEnter(ev: DragEvent) {
  if (!hasFiles(ev)) return;
  ev.preventDefault();
  dragDepth += 1;
  dropping.value = true;
}
function onWinDragOver(ev: DragEvent) {
  if (!hasFiles(ev)) return;
  ev.preventDefault();
  if (ev.dataTransfer) ev.dataTransfer.dropEffect = "copy";
  dropping.value = true;
}
function onWinDragLeave(ev: DragEvent) {
  if (!hasFiles(ev)) return;
  dragDepth = Math.max(0, dragDepth - 1);
  if (dragDepth === 0) dropping.value = false;
}
function onWinDrop(ev: DragEvent) {
  ev.preventDefault();
  dragDepth = 0;
  dropping.value = false;
  if (tauriDropHandled) return;
  const file = imageFromEvent(ev);
  if (file) void store.openImportFromFile(file);
}

function onPaste(ev: ClipboardEvent) {
  const file = imageFromEvent(ev);
  if (!file) return;
  ev.preventDefault();
  void store.openImportFromFile(file);
}

let unlistenTauri: (() => void) | undefined;
onMounted(() => {
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
        tauriDropHandled = true;
        window.setTimeout(() => {
          tauriDropHandled = false;
        }, 400);
        const paths = event.payload.paths ?? [];
        const path = paths.find((item) => /\.(png|jpe?g|webp|gif|bmp)$/i.test(item));
        if (path) void store.openImportFromPath(path);
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
  window.removeEventListener("paste", onPaste);
  window.removeEventListener("dragenter", onWinDragEnter, true);
  window.removeEventListener("dragover", onWinDragOver, true);
  window.removeEventListener("dragleave", onWinDragLeave, true);
  window.removeEventListener("drop", onWinDrop, true);
  unlistenTauri?.();
});

function onStageClick(ev: MouseEvent, id: string) {
  const el = ev.currentTarget as HTMLElement;
  const rect = el.getBoundingClientRect();
  store.updateCharacter(id, {
    x: Math.min(1, Math.max(0, (ev.clientX - rect.left) / rect.width)),
    y: Math.min(1, Math.max(0, (ev.clientY - rect.top) / rect.height)),
    useCoords: true,
  });
}

function runGenerate() {
  void store.generate(store.i2iImage ? "img2img" : "txt2img");
}

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
        <textarea
          v-if="promptTab === 'prompt'"
          v-model="store.params.positivePrompt"
          placeholder="Enter your prompt here..."
        />
        <textarea
          v-else
          v-model="store.params.negativePrompt"
          placeholder="lowres, worst quality..."
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
            <button class="icon-sq tiny" type="button" :class="{ on: store.customPositions }" @click="store.setPositionMode(!store.customPositions)">
              <NaiIcon name="grid" :size="14" />
            </button>
          </div>
        </div>

        <div
          v-if="store.customPositions && store.characters.length"
          class="stage"
          :style="{ aspectRatio: `${store.params.width} / ${store.params.height}` }"
          @mousemove="dragging && onStageClick($event, dragging)"
          @mouseup="dragging = null"
          @mouseleave="dragging = null"
        >
          <button
            v-for="(c, i) in store.characters"
            :key="c.id"
            class="marker"
            type="button"
            :style="{ left: `${c.x * 100}%`, top: `${c.y * 100}%` }"
            @mousedown.stop="dragging = c.id"
          >
            {{ i + 1 }}
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
          <textarea
            v-if="(charTab[c.id] || 'prompt') === 'prompt'"
            :value="c.prompt"
            @input="store.updateCharacter(c.id, { prompt: ($event.target as HTMLTextAreaElement).value })"
          />
          <textarea
            v-else
            :value="c.negativePrompt"
            @input="store.updateCharacter(c.id, { negativePrompt: ($event.target as HTMLTextAreaElement).value })"
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

        <label class="stream-toggle">
          <input v-model="store.settings.streamPreviewEnabled" type="checkbox" @change="store.saveSettings()" />
          流式预览
        </label>
        <button class="generate" type="button" :disabled="store.busy" @click="runGenerate">
          <span>{{ store.busy ? "Generating…" : generateLabel }}</span>
          <span class="cost"><NaiIcon name="anlas" :size="13" />{{ anlas }}</span>
        </button>
      </section>
      </div>
    </section>

    <section class="center">
      <div v-if="store.busy" class="stream-stage">
        <img v-if="store.genPreview" :src="store.genPreview" alt="" />
        <div v-else class="stream-wait">{{ store.genPhase === "waiting" ? "正在连接 NovelAI…" : "Generating…" }}</div>
        <div class="stream-status">
          <strong>{{ store.genPhase === "saving" ? "正在保存" : store.settings.streamPreviewEnabled ? "流式生成中" : "正在生成" }}</strong>
          <span>{{ Math.round(store.genProgress * 100) }}% · {{ store.genStep }}/{{ store.genSteps }} steps</span>
          <div class="bar"><i :style="{ width: `${Math.max(4, Math.round(store.genProgress * 100))}%` }" /></div>
        </div>
      </div>
      <img v-else-if="store.previewUrl" :src="store.previewUrl" alt="preview" />
      <StarterGallery v-else />
    </section>

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
  flex: 1;
  min-width: 0;
  overflow: auto;
  display: flex;
  align-items: flex-start;
  justify-content: center;
  background: var(--bg0);
}
.center img { max-width: 100%; max-height: 100%; object-fit: contain; margin: auto; }
.stream-stage {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 16px;
  padding: 24px;
}
.stream-stage img {
  max-height: calc(100% - 88px);
  border-radius: 8px;
  box-shadow: 0 18px 40px rgba(0, 0, 0, 0.35);
}
.stream-wait {
  color: var(--muted);
  font-family: Eczar, serif;
  font-size: 28px;
}
.stream-status {
  width: min(420px, 90%);
  display: grid;
  gap: 6px;
  color: #fff;
}
.stream-status strong { font-size: 15px; }
.stream-status span { color: var(--muted); font-size: 12px; }
.stream-status .bar {
  height: 6px;
  border-radius: 99px;
  background: #22253f;
  overflow: hidden;
}
.stream-status .bar i {
  display: block;
  height: 100%;
  background: #f5f3c2;
  transition: width 0.18s ease;
}
.stream-toggle {
  display: flex;
  align-items: center;
  gap: 8px;
  color: #fff;
  font-size: 13px;
}
.stream-toggle input { accent-color: #f5f3c2; width: 16px; height: 16px; }
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
.pos { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; margin: 8px 0; font-size: 13px; color: var(--muted); }
.seg { display: flex; gap: 6px; margin-left: auto; }
.seg button {
  background: var(--bg2); border: 1px solid var(--bg3); border-radius: 4px; color: #fff; padding: 6px 10px;
}
.seg button.on, .icon-sq.tiny.on {
  background: var(--chip); color: var(--bg0); border-color: var(--chip);
}
.stage {
  position: relative; width: 100%; max-width: 220px; margin: 8px auto;
  background: var(--bg0); border: 1px dashed var(--bg3); border-radius: 6px;
}
.marker {
  position: absolute; transform: translate(-50%, -50%);
  width: 20px; height: 20px; border-radius: 50%; border: 0;
  background: var(--heading); color: var(--bg0); font-size: 11px; font-weight: 700;
}
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
.generate {
  width: 100%; display: flex; align-items: center; justify-content: space-between;
  background: #f5f3c2; color: var(--bg0); border: 0; border-radius: 8px; padding: 10px 10px 10px 14px; font-weight: 700;
  margin-top: 8px;
}
.generate:disabled { opacity: 0.55; }
.cost {
  display: inline-flex; align-items: center; gap: 4px;
  background: #0e0f21; color: #fff; border-radius: 8px; padding: 6px 10px; font-size: 13px;
}
.ai-acts { display: flex; gap: 4px; }
</style>
